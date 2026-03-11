# DB 모니터링 분석 가이드

이 문서는 데이터베이스 모니터링 데이터를 분석하여 쿼리 성능, 락, 커넥션 문제를 진단하고 해결하는 방법을 설명합니다.

---

## 1. 이상 징후 감지

### 1.1 모니터링해야 할 핵심 지표

| 지표 | 정상 범위 | 주의 | 심각 |
|------|----------|------|------|
| 쿼리 평균 응답시간 | < 10ms | 10-50ms | > 50ms |
| 슬로우 쿼리 비율 | < 0.1% | 0.1-1% | > 1% |
| 커넥션 풀 사용률 | < 70% | 70-90% | > 90% |
| 버퍼 캐시 적중률 | > 95% | 90-95% | < 90% |
| 락 대기 시간 | < 10ms | 10-100ms | > 100ms |
| 락 대기 횟수 | < 10/min | 10-50/min | > 50/min |
| 활성 세션 | < 70% max | 70-90% | > 90% |
| 복제 지연 (Replica) | < 1s | 1-10s | > 10s |

### 1.2 이상 징후 패턴

**패턴 A: 특정 쿼리만 느림**
```
증상: UPDATE orders SET ... 쿼리만 500ms
의심: 인덱스 누락, 락 대기, 데이터량 증가
```

**패턴 B: 특정 시간대 전체 느림**
```
증상: 매시 정각 전체 쿼리 느려짐
의심: 배치 작업, 통계 수집, 백업
```

**패턴 C: 커넥션 풀 고갈 반복**
```
증상: "Connection not available" 에러 반복
의심: 커넥션 누수, 쿼리 지연, 풀 크기 부족
```

**패턴 D: 데드락 발생**
```
증상: Deadlock found when trying to get lock
의심: 트랜잭션 순서, 락 타임아웃, 동시성 문제
```

---

## 2. 원인 분석 플로우

### 2.1 쿼리 느림 → 원인 찾기

```
쿼리 느림
    │
    ▼
실행계획 확인
    │
┌───┴────┐
풀스캔?  인덱스
│        사용?
▼        │
인덱스   ┌───┴───┐
추가     적중률  락
         낮음?   대기?
         │       │
         ▼       ▼
      캐시    트랜잭션
      문제    대기
```

**분석 명령어:**

```bash
# 1. 슬로우 쿼리 목록
$ whatap mxql --pcode 12345 "CATEGORY db_slow_query
TAGLOAD
SELECT [query_text, count, avg_time, max_time, rows_examined]
FILTER { avg_time > 50 }
LIMIT 20"

query_text                                  count  avg_time  rows_examined
SELECT * FROM orders WHERE status = ?      1,234  320ms     1,500,000    ← 풀스캔 의심
SELECT * FROM products WHERE category...   892    180ms     890,000
UPDATE inventory SET stock = ? WHERE...    567    450ms     45

# 해석: orders 테이블 쿼리가 150만 행 검사
# → 인덱스 미사용 가능성
```

```bash
# 2. 해당 테이블 인덱스 확인
$ whatap mxql --pcode 12345 "CATEGORY db_index_usage
TAGLOAD
SELECT [index_name, access_count, rows_read]
FILTER { table_name == 'orders' }
LIMIT 10"

index_name      access_count  rows_read
PRIMARY         234,567       234,567
idx_user_id     89,234        89,234
(없음 - status)  0             0         ← status 컬럼 인덱스 없음

# 해석: status 컬럼에 인덱스 없음
# → WHERE status = ? 조건이 풀스캔 유발
```

```bash
# 3. 락 대기 확인
$ whatap mxql --pcode 12345 "CATEGORY db_lock_wait
TAGLOAD
SELECT [lock_type, table_name, wait_count, avg_wait_time]
FILTER { wait_count > 0 }
LIMIT 10"

lock_type   table_name   wait_count  avg_wait_time
ROW         inventory    234         45ms
TABLE       orders       45          120ms

# 해석: inventory 테이블 ROW 락 대기 234건
# → 동시 업데이트 충돌
```

### 2.2 커넥션 풀 고갈 → 원인 찾기

```
커넥션 풀 고갈
      │
      ▼
장기 실행 쿼리?
      │
  ┌───┴───┐
  YES     NO
  │       │
  ▼       ▼
쿼리    커넥션
최적화  누수?
        │
    ┌───┴───┐
    YES     NO
    │       │
    ▼       ▼
커넥션  풀 크기
반납   증설
누락
```

**분석 명령어:**

```bash
# 1. 커넥션 풀 상태
$ whatap spot --pcode 12345 --keys conn_pool_active,conn_pool_idle,conn_pool_max

conn_pool_active: 50
conn_pool_idle: 0          ← 문제
conn_pool_max: 50

# 해석: 모든 커넥션 사용 중, 여유 없음
```

```bash
# 2. 장기 실행 쿼리 확인
$ whatap mxql --pcode 12345 "CATEGORY db_active_sessions
TAGLOAD
SELECT [query_text, duration, state, user]
FILTER { duration > 5000 }
LIMIT 20"

query_text                                duration  state
SELECT * FROM audit_logs WHERE ...        45,230ms  executing
UPDATE user_stats SET ...                 23,456ms  executing
DELETE FROM sessions WHERE expires...     12,890ms  executing

# 해석: audit_logs 조회에 45초 소요 중
# → 장기 실행 쿼리가 커넥션 점유
```

```bash
# 3. 세션별 커넥션 시간
$ whatap mxql --pcode 12345 "CATEGORY db_sessions
TAGLOAD
SELECT [session_id, user, host, connection_time, state]
LIMIT 20"

session_id  user     host          connection_time  state
12345       app      10.0.1.5      1,234,567ms      Sleep    ← 20분 이상
12346       app      10.0.1.6      987,654ms        Sleep
12347       batch    10.0.1.10     2,345,678ms      Query

# 해석: Sleep 상태 세션이 커넥션 장시간 점유
# → 커넥션 누수 의심
```

### 2.3 데드락 발생 → 원인 찾기

```
데드락 발생
     │
     ▼
관련 테이블?
     │
┌────┴────┐
단일      다중
│         │
▼         ▼
자기     교차
참조     락
│         │
▼         ▼
트리거   접근
문제    순서
```

**분석 명령어:**

```bash
# 1. 데드락 로그 확인
$ whatap log search --pcode 12345 --category db_error_log --keyword "deadlock" --duration 24h

[2024-01-15 10:15:23] ERROR Deadlock found when trying to get lock
  Transaction 1: holding lock on orders, waiting for lock on inventory
  Transaction 2: holding lock on inventory, waiting for lock on orders

# 해석: orders ↔ inventory 간 교차 락 대기
# → 트랜잭션 접근 순서 문제
```

```bash
# 2. 데드락 관련 쿼리
$ whatap mxql --pcode 12345 "CATEGORY db_deadlock
TAGLOAD
SELECT [query_text, table_name, lock_type]
LIMIT 10"

query_text                              table_name   lock_type
UPDATE orders SET status = ? WHERE...   orders       ROW
UPDATE inventory SET stock = ? WHERE... inventory    ROW

# 해석: orders와 inventory를 동시에 업데이트하는 트랜잭션 간 충돌
```

### 2.4 캐시 적중률 저하 → 원인 찾기

```
캐시 적중률 저하
       │
       ▼
   버퍼 크기?
       │
   ┌───┴───┐
   작음    충분
   │       │
   ▼       ▼
증설    쿼리
       패턴?
       │
   ┌───┴───┐
   풀스캔  대량
   많음    조회
```

**분석 명령어:**

```bash
# 1. 캐시 적중률 확인
$ whatap spot --pcode 12345 --keys cache_hit_ratio,buffer_pool_size

cache_hit_ratio: 78.5%     ← 낮음 (정상 > 95%)
buffer_pool_size: 4GB

# 해석: 적중률 78.5%, 너무 낮음
```

```bash
# 2. 적중률 추이
$ whatap stat query --pcode 12345 --category db_cache --field hit_ratio --duration 24h

time                  value
2024-01-15 00:00      96.2%
2024-01-15 08:00      94.5%
2024-01-15 12:00      82.3%   ← 하락 시작
2024-01-15 14:00      78.5%

# 해석: 12시부터 적중률 하락
# → 해당 시점 데이터/쿼리 패턴 변화 확인
```

```bash
# 3. 풀스캔 발생 확인
$ whatap mxql --pcode 12345 "CATEGORY db_table_scan
TAGLOAD
SELECT [table_name, scan_count, rows_read]
LIMIT 10"

table_name      scan_count  rows_read
audit_logs      1,234       45,678,900    ← 대량 풀스캔
user_activity   567         12,345,678

# 해석: audit_logs 테이블 풀스캔이 4,500만 행 읽기
# → 캐시 오염시키는 대량 풀스캔 발생
```

---

## 3. 실제 트러블슈팅 케이스

### 케이스 1: 주문 생성 API 간헐적 타임아웃

**증상:**
- 주문 생성 API 5초 타임아웃 간헐 발생
- 하루 50-100건 발생

**분석 과정:**

```bash
# Step 1: 느린 쿼리 확인
$ whatap mxql --pcode 12345 "CATEGORY db_slow_query
TAGLOAD
SELECT [query_text, avg_time, max_time]
FILTER { query_text like '%orders%' }
LIMIT 10"

query_text                                  avg_time  max_time
INSERT INTO orders (...)                    234ms     5.2s    ← 문제
UPDATE inventory SET stock = stock - ?...    45ms      120ms

# Step 2: 락 대기 확인
$ whatap mxql --pcode 12345 "CATEGORY db_lock_wait
TAGLOAD
SELECT [table_name, wait_count, avg_wait_time, max_wait_time]
LIMIT 10"

table_name   wait_count  avg_wait_time  max_wait_time
inventory    234         45ms           4.8s          ← 여기!

# Step 3: 인벤토리 업데이트 패턴 분석
$ whatap mxql --pcode 12345 "CATEGORY db_query_stats
TAGLOAD
SELECT [query_text, count]
FILTER { query_text like '%inventory%' }
LIMIT 10"

query_text                                    count
UPDATE inventory SET stock = stock - ?        12,345   ← 동시 업데이트 많음
SELECT stock FROM inventory WHERE id = ?      12,345

# 해석: 동일 상품 재고 동시 업데이트 시 ROW 락 충돌
# → 최대 4.8초까지 대기
```

**원인:** 인기 상품 재고 동시 업데이트로 ROW 락 대기

**해결:**
```sql
-- Before (락 대기)
UPDATE inventory SET stock = stock - ? WHERE product_id = ?;

-- After 1: 비관적 락 타임아웃 설정
SET innodb_lock_wait_timeout = 2;

-- After 2: 낙관적 락으로 변경
UPDATE inventory
SET stock = stock - ?, version = version + 1
WHERE product_id = ? AND version = ?;

-- After 3: Redis로 재고 선점 후 비동기 DB 반영
```

---

### 케이스 2: 매시 정각 DB 응답 지연

**증상:**
- 매시 정각(00분)에 1분간 DB 쿼리 10배 느려짐
- 애플리케이션 에러 급증

**분석 과정:**

```bash
# Step 1: 시간대별 응답시간
$ whatap stat query --pcode 12345 --category db_counter --field resp_time --duration 24h

time                  value
...
2024-01-15 09:55      8ms
2024-01-15 10:00      125ms   ← 스파이크
2024-01-15 10:01      98ms
2024-01-15 10:05      10ms

# Step 2: 정각 실행 쿼리 확인
$ whatap mxql --pcode 12345 "CATEGORY db_slow_query
TAGLOAD
SELECT [query_text, avg_time, @timestamp]
FILTER { @timestamp >= 1705314000000 AND @timestamp < 1705314060000 }
LIMIT 10"

query_text                                    avg_time  @timestamp
ANALYZE TABLE orders                          45,000ms  10:00:00
DELETE FROM user_sessions WHERE expires...    23,000ms  10:00:05

# 해석: ANALYZE TABLE이 정각에 실행되어 45초 소요
# → 통계 수집이 테이블 락 유발
```

**원인:** 시스템 예약 작업(ANALYZE TABLE)이 정각에 실행

**해결:**
1. ANALYZE TABLE 실행 시간 변경 (새벽 3시 등)
2. Online DDL 사용 (MySQL 5.6+)
3. pt-online-schema-change 등 툴 사용

---

### 케이스 3: 캐시 적중률 60%로 저하

**증상:**
- 버퍼 캐시 적중률이 95% → 60%로 저하
- 전체 쿼리 응답시간 증가

**분석 과정:**

```bash
# Step 1: 적중률 변화 시점
$ whatap stat query --pcode 12345 --category db_cache --field hit_ratio --duration 7d

# 3일 전부터 지속적 하락 확인

# Step 2: 데이터 크기 변화
$ whatap mxql --pcode 12345 "CATEGORY db_table_size
TAGLOAD
SELECT [table_name, size_mb, rows]
LIMIT 10"

table_name      size_mb   rows
audit_logs      45,000    120,000,000   ← 급증
orders          12,000    15,000,000
products        500       100,000

# Step 3: 버퍼 풀 크기 확인
$ whatap spot --pcode 12345 --keys buffer_pool_size,buffer_pool_used

buffer_pool_size: 8GB
buffer_pool_used: 8GB

# 해석: audit_logs가 45GB로 커짐
# → 8GB 버퍼 풀로 감당 불가
```

**원인:** audit_logs 테이블 급증으로 버퍼 풀 부족

**해결:**
1. 버퍼 풀 크기 증설 (8GB → 16GB)
2. audit_logs 테이블 파티셔닝
3. 오래된 audit_logs 아카이빙/삭제
4. audit_logs 쿼리 최적화 (필요한 컬럼만, 기간 제한)

---

### 케이스 4: 커넥션 풀 고갈로 서비스 중단

**증상:**
- "Connection not available" 에러로 서비스 중단
- 애플리케이션 재시작 후 일시 회복

**분석 과정:**

```bash
# Step 1: 커넥션 상태
$ whatap spot --pcode 12345 --keys conn_pool_active,conn_pool_max

conn_pool_active: 100
conn_pool_max: 100      ← 꽉 참

# Step 2: 장기 실행 세션
$ whatap mxql --pcode 12345 "CATEGORY db_sessions
TAGLOAD
SELECT [query_text, duration, state, program_name]
FILTER { duration > 10000 }
LIMIT 20"

query_text                          duration   state    program_name
SELECT * FROM reports WHERE ...     180,000ms  execute  report-generator
SELECT * FROM exports WHERE ...     120,000ms  execute  export-worker
SHOW PROCESSLIST                    5,000ms    execute  monitoring

# Step 3: 커넥션 누수 의심 지점
$ whatap log search --pcode 12345 --keyword "connection" --level WARN --duration 1h

[2024-01-15 10:15:23] WARN Connection not returned to pool (query timeout)
[2024-01-15 10:15:45] WARN Connection not returned to pool (query timeout)

# 해석: 대용량 리포트 쿼리가 3분 실행
# → 타임아웃 후 커넥션이 반환되지 않고 누수
```

**원인:** 대용량 리포트 쿼리 타임아웃 시 커넥션 미반환

**해결:**
1. 리포트 쿼리 최적화 (페이징, 인덱싱)
2. 커넥션 풀 타임아웃 설정
3. 대용량 쿼리는 읽기 전용 복제본 사용
4. 커넥션 누수 감지 및 자동 반환 로직 추가

---

## 4. 핵심 분석 체크리스트

### 쿼리 느릴 때

- [ ] 실행계획에 풀스캔? → 인덱스 추가
- [ ] 인덱스 있어도 느림? → 카디널리티, 통계 확인
- [ ] 락 대기? → 동시성 문제
- [ ] rows_examined >> rows_sent? → 불필요한 데이터 읽기
- [ ] 특정 시간대만? → 배치/통계 작업 영향

### 커넥션 풀 문제 시

- [ ] 장기 실행 쿼리? → 쿼리 최적화
- [ ] 커넥션 누수? → finally 블록에서 반납 확인
- [ ] 풀 크기 부족? → 증설 검토
- [ ] Sleep 세션 다수? → wait_timeout 조정
- [ ] 특정 앱에서만? → 해당 앱 커넥션 관리 확인

### 락/데드락 시

- [ ] 교차 락? → 트랜잭션 접근 순서 통일
- [ ] 락 타임아웃 짧음? → 적절히 조정
- [ ] 동시 업데이트 많음? → 배치 처리 또는 큐 사용
- [ ] 트리거/외래키? → 연쇄 락 확인

### 캐시 문제 시

- [ ] 버퍼 풀 크기 부족? → 증설
- [ ] 대량 풀스캔? → 인덱스 추가 또는 쿼리 수정
- [ ] 데이터 급증? → 파티셔닝/아카이빙
- [ ] 캐시 워밍업 필요? → 서비스 시작 시 프리로드

---

## 5. 빠른 진단 명령어

```bash
# DB 전체 상태
$ whatap spot --pcode 12345

# 슬로우 쿼리
$ whatap mxql --pcode 12345 "CATEGORY db_slow_query TAGLOAD SELECT [query_text, avg_time] FILTER { avg_time > 100 } LIMIT 20"

# 락 대기
$ whatap mxql --pcode 12345 "CATEGORY db_lock_wait TAGLOAD SELECT [table_name, wait_count, avg_wait_time] FILTER { wait_count > 0 } LIMIT 10"

# 커넥션 상태
$ whatap spot --pcode 12345 --keys conn_pool_active,conn_pool_max

# 장기 실행 세션
$ whatap mxql --pcode 12345 "CATEGORY db_sessions TAGLOAD SELECT [query_text, duration] FILTER { duration > 5000 } LIMIT 10"

# 캐시 적중률
$ whatap stat query --pcode 12345 --category db_cache --field hit_ratio --duration 1h

# 데드락 로그
$ whatap log search --pcode 12345 --category db_error_log --keyword "deadlock" --duration 24h
```
