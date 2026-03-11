# WhatAp CLI - DB 모니터링 분석 예시

데이터베이스 모니터링 데이터 분석 시나리오와 CLI 사용 예시입니다.

## 사전 요구사항

```bash
# 로그인 (이메일/비밀번호)
whatap login -e user@example.com -p 'password'

# 또는 API 키로 로그인 (CI/CD)
whatap login --api-key <key> --pcode <pcode>

# DB 프로젝트 확인
whatap projects --filter DB
```

---

## 1. 실시간 메트릭 조회 (Spot)

### 1.1 전체 DB 메트릭

```bash
# 모든 실시간 메트릭
whatap spot --pcode 12345
```

**출력 예시:**

```bash
$ whatap spot --pcode 12345
```
```
Active Sessions: 45
TPS: 1,245
Query Response Time: 12ms avg
Connection Pool: 8/20 used
Cache Hit Ratio: 95.2%
Lock Wait: 3
Disk Read: 156 KB/s
Disk Write: 89 KB/s
Replication Lag: 0ms
```

### 1.2 특정 메트릭만 조회

```bash
# 세션과 TPS
whatap spot --pcode 12345 --keys active_sessions,tps

# 커넥션 풀
whatap spot --pcode 12345 --keys conn_pool_used,conn_pool_max

# 캐시 적중률
whatap spot --pcode 12345 --keys cache_hit_ratio
```

**출력 예시:**

```bash
$ whatap spot --pcode 12345 --keys active_sessions,tps,cache_hit_ratio
```
```
active_sessions: 45
tps: 1,245
cache_hit_ratio: 95.2%
```

### 1.3 JSON 출력

```bash
$ whatap spot --pcode 12345 --keys tps,resp_time --json
```
```json
{
  "tps": 1245,
  "resp_time": 12,
  "timestamp": 1705312800000
}
```

---

## 2. 시계열 통계 조회 (Stat)

### 2.1 TPS 추이

```bash
# 지난 1시간 TPS
whatap stat query --pcode 12345 \
  --category db_counter \
  --field tps \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category db_counter --field tps --duration 1h
```
```
time                  value
2024-01-15 10:00      1,125
2024-01-15 10:05      1,342
2024-01-15 10:10      1,089
2024-01-15 10:15      1,456
2024-01-15 10:20      1,298
2024-01-15 10:25      1,512
```

### 2.2 쿼리 응답시간

```bash
# 평균 응답시간
whatap stat query --pcode 12345 \
  --category db_counter \
  --field resp_time \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category db_counter --field resp_time --duration 1h
```
```
time                  value
2024-01-15 10:00      12ms
2024-01-15 10:05      15ms
2024-01-15 10:10      8ms
2024-01-15 10:15      23ms
2024-01-15 10:20      11ms
```

### 2.3 커넥션 풀 사용률

```bash
# 커넥션 풀 추이
whatap stat query --pcode 12345 \
  --category db_pool \
  --field active \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category db_pool --field active --duration 1h
```
```
time                  value
2024-01-15 10:00      8
2024-01-15 10:05      12
2024-01-15 10:10      6
2024-01-15 10:15      18
2024-01-15 10:20      15
```

### 2.4 캐시 적중률

```bash
# 버퍼 캐시 적중률
whatap stat query --pcode 12345 \
  --category db_cache \
  --field hit_ratio \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category db_cache --field hit_ratio --duration 1h
```
```
time                  value
2024-01-15 10:00      95.2%
2024-01-15 10:05      94.8%
2024-01-15 10:10      96.1%
2024-01-15 10:15      93.5%
2024-01-15 10:20      95.7%
```

---

## 3. MXQL 쿼리

### 3.1 슬로우 쿼리 분석

```bash
# 느린 쿼리 목록
whatap mxql --pcode 12345 "CATEGORY db_slow_query
TAGLOAD
SELECT [query_text, count, avg_time, max_time, rows_examined]
FILTER { avg_time > 100 }
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_slow_query TAGLOAD SELECT [query_text, count, avg_time, max_time] FILTER { avg_time > 100 } LIMIT 10"
```
```
query_text                                          count  avg_time  max_time
SELECT * FROM orders WHERE user_id = ? AND ...      1,234  520ms     2,340ms
SELECT * FROM products WHERE category_id IN (...)    892    345ms     1,890ms
UPDATE inventory SET stock = ? WHERE product_id = ?  567    280ms     1,456ms
SELECT COUNT(*) FROM audit_logs WHERE created_at > ? 234    1,230ms   5,670ms
DELETE FROM sessions WHERE expires_at < ?            45     890ms     2,100ms
```

### 3.2 쿼리 패턴 분석

```bash
# 쿼리 타입별 통계
whatap mxql --pcode 12345 "CATEGORY db_query_stats
TAGLOAD
SELECT [query_type, count, avg_time, total_time]
LIMIT 100"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_query_stats TAGLOAD SELECT [query_type, count, avg_time, total_time] LIMIT 10"
```
```
query_type  count     avg_time  total_time
SELECT      523,456   15ms      7,851s
INSERT      89,234    8ms       714s
UPDATE      34,567    23ms      795s
DELETE      12,345    12ms      148s
JOIN        8,901     45ms      401s
```

### 3.3 테이블별 통계

```bash
# 테이블 접근 통계
whatap mxql --pcode 12345 "CATEGORY db_table_access
TAGLOAD
SELECT [table_name, read_count, write_count, rows_read, rows_written]
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_table_access TAGLOAD SELECT [table_name, read_count, write_count, rows_read] LIMIT 10"
```
```
table_name      read_count  write_count  rows_read
orders          234,567     89,234       1,234,567
products        456,789     12,345       2,345,678
users           123,456     34,567       567,890
inventory       89,234      78,901       456,789
sessions        567,890     567,890      890,123
```

### 3.4 락 대기 분석

```bash
# 락 대기 발생
whatap mxql --pcode 12345 "CATEGORY db_lock_wait
TAGLOAD
SELECT [lock_type, table_name, wait_count, avg_wait_time, max_wait_time]
FILTER { wait_count > 0 }
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_lock_wait TAGLOAD SELECT [lock_type, table_name, wait_count, avg_wait_time, max_wait_time] FILTER { wait_count > 0 } LIMIT 10"
```
```
lock_type   table_name   wait_count  avg_wait_time  max_wait_time
ROW         inventory    234         45ms           890ms
TABLE       orders       45          120ms          1,234ms
ROW         products     89          23ms           456ms
METADATA    users        12          200ms          567ms
```

### 3.5 인덱스 사용률

```bash
# 인덱스 사용 통계
whatap mxql --pcode 12345 "CATEGORY db_index_usage
TAGLOAD
SELECT [index_name, table_name, access_count, rows_read]
LIMIT 100"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_index_usage TAGLOAD SELECT [index_name, table_name, access_count, rows_read] LIMIT 10"
```
```
index_name          table_name  access_count  rows_read
PRIMARY             orders      456,789       1,234,567
idx_user_id         orders      234,567       567,890
idx_product_cat     products    123,456       345,678
idx_created_at      orders      89,234        234,567
idx_email           users       67,890        67,890
```

### 3.6 미사용 인덱스

```bash
# 사용되지 않는 인덱스
whatap mxql --pcode 12345 "CATEGORY db_index_unused
TAGLOAD
SELECT [index_name, table_name, size_mb]
FILTER { access_count == 0 }
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY db_index_unused TAGLOAD SELECT [index_name, table_name, size_mb] FILTER { access_count == 0 } LIMIT 10"
```
```
index_name          table_name  size_mb
idx_old_field       orders      45.2
idx_deprecated      products    23.8
idx_backup          users       12.4
idx_temp            inventory   8.9
```

---

## 4. DB 로그 분석

### 4.1 에러 로그 검색

```bash
# DB 에러 로그
whatap log search --pcode 12345 \
  --category db_error_log \
  --level ERROR \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --category db_error_log --level ERROR --duration 1h
```
```
8 ERROR log entries

[2024-01-15 10:15:23] ERROR [InnoDB] Lock wait timeout exceeded
[2024-01-15 10:23:45] ERROR [InnoDB] Deadlock found when trying to get lock
[2024-01-15 10:31:12] ERROR [Server] Too many connections
[2024-01-15 10:42:08] ERROR [InnoDB] Cannot allocate memory for buffer pool
```

### 4.2 데드락 로그

```bash
# 데드락 발생 검색
whatap log search --pcode 12345 \
  --category db_error_log \
  --keyword "deadlock" \
  --duration 24h
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --category db_error_log --keyword "deadlock" --duration 24h
```
```
5 log entries matching "deadlock"

[2024-01-15 08:15:23] ERROR [InnoDB] Deadlock found when trying to get lock; try restarting transaction
[2024-01-15 12:34:56] ERROR [InnoDB] Deadlock found when trying to get lock; try restarting transaction
[2024-01-15 15:23:12] ERROR [InnoDB] Deadlock found when trying to get lock; try restarting transaction
```

### 4.3 커넥션 관련 로그

```bash
# 커넥션 에러 검색
whatap log search --pcode 12345 \
  --category db_error_log \
  --keyword "connection" \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --category db_error_log --keyword "connection" --duration 1h
```
```
3 log entries matching "connection"

[2024-01-15 10:15:23] WARN [Server] Too many connections (max: 100, current: 98)
[2024-01-15 10:23:45] ERROR [Server] Host '192.168.1.100' blocked because of many connection errors
[2024-01-15 10:31:12] WARN [Server] Connection timeout for client 'app-server-01'
```

---

## 5. 알림 관리

### 5.1 DB 알림 생성

```bash
# 슬로우 쿼리 알림
whatap alert create --pcode 12345 \
  --title "Slow Query Alert" \
  --category db_counter \
  --warning "avg_query_time > 100" \
  --critical "avg_query_time > 500" \
  --message "Slow query detected: {{value}}ms"

# 커넥션 풀 알림
whatap alert create --pcode 12345 \
  --title "Connection Pool Alert" \
  --category db_pool \
  --warning "pool_usage > 80" \
  --critical "pool_usage > 95" \
  --message "Connection pool usage high: {{value}}%"

# 락 대기 알림
whatap alert create --pcode 12345 \
  --title "Lock Wait Alert" \
  --category db_lock \
  --warning "lock_wait_count > 10" \
  --critical "lock_wait_count > 50" \
  --message "Lock wait detected: {{value}} locks waiting"

# 캐시 적중률 알림
whatap alert create --pcode 12345 \
  --title "Low Cache Hit Ratio" \
  --category db_cache \
  --warning "hit_ratio < 90" \
  --critical "hit_ratio < 80" \
  --message "Cache hit ratio is low: {{value}}%"
```

**출력 예시:**

```bash
$ whatap alert create --pcode 12345 --title "Slow Query Alert" --category db_counter --warning "avg_query_time > 100" --message "Slow query detected: {{value}}ms"
```
```
Alert created successfully
ID: 2001
Title: Slow Query Alert
Category: db_counter
Status: enabled
```

### 5.2 알림 목록

```bash
$ whatap alert list --pcode 12345
```
```
ID    Title                    Category      Status    Conditions
2001  Slow Query Alert         db_counter    enabled   avg_query_time > 100ms
2002  Connection Pool Alert    db_pool       enabled   pool_usage > 80%
2003  Lock Wait Alert          db_lock       enabled   lock_wait_count > 10
2004  Low Cache Hit Ratio      db_cache      enabled   hit_ratio < 90%
2005  Replication Lag Alert    db_replica    disabled  lag > 1000ms
```

---

## 6. 분석 시나리오

### 시나리오 1: 슬로우 쿼리 원인 분석

```bash
# 1단계: 실시간 상태 확인
whatap spot --pcode 12345 --keys tps,resp_time,active_sessions

# 2단계: 느린 쿼리 목록 확인
whatap mxql --pcode 12345 "CATEGORY db_slow_query TAGLOAD SELECT [query_text, count, avg_time, max_time] FILTER { avg_time > 100 } LIMIT 20"

# 3단계: 해당 테이블 인덱스 확인
whatap mxql --pcode 12345 "CATEGORY db_index_usage TAGLOAD SELECT [index_name, access_count] LIMIT 50"

# 4단계: 락 대기 확인
whatap mxql --pcode 12345 "CATEGORY db_lock_wait TAGLOAD SELECT [lock_type, table_name, wait_count, avg_wait_time] FILTER { wait_count > 0 } LIMIT 20"
```

### 시나리오 2: 커넥션 풀 문제 분석

```bash
# 1단계: 커넥션 풀 상태
whatap spot --pcode 12345 --keys conn_pool_used,conn_pool_max

# 2단계: 커넥션 추이 확인
whatap stat query --pcode 12345 --category db_pool --field active --duration 1h

# 3단계: 활성 세션 확인
whatap mxql --pcode 12345 "CATEGORY db_sessions TAGLOAD SELECT [session_id, user, host, state, query_time] LIMIT 50"

# 4단계: 커넥션 관련 에러 로그
whatap log search --pcode 12345 --category db_error_log --keyword "connection" --duration 1h
```

### 시나리오 3: 데드락 분석

```bash
# 1단계: 데드락 로그 확인
whatap log search --pcode 12345 --category db_error_log --keyword "deadlock" --duration 24h

# 2단계: 락 대기 현황
whatap mxql --pcode 12345 "CATEGORY db_lock_wait TAGLOAD SELECT [lock_type, table_name, wait_count, avg_wait_time] LIMIT 50"

# 3단계: 관련 쿼리 확인
whatap mxql --pcode 12345 "CATEGORY db_slow_query TAGLOAD SELECT [query_text, count] FILTER { query_text like 'UPDATE' } LIMIT 50"

# 4단계: 트랜잭션 패턴 확인
whatap mxql --pcode 12345 "CATEGORY db_transaction TAGLOAD SELECT [transaction_type, count, avg_duration] LIMIT 50"
```

### 시나리오 4: 성능 저하 분석

```bash
# 1단계: 응답시간 추이
whatap stat query --pcode 12345 --category db_counter --field resp_time --duration 1h

# 2단계: 캐시 적중률 확인
whatap stat query --pcode 12345 --category db_cache --field hit_ratio --duration 1h

# 3단계: 미사용 인덱스 확인
whatap mxql --pcode 12345 "CATEGORY db_index_unused TAGLOAD SELECT [index_name, table_name, size_mb] LIMIT 50"

# 4단계: 테이블 크기 확인
whatap mxql --pcode 12345 "CATEGORY db_table_size TAGLOAD SELECT [table_name, size_mb, rows] LIMIT 50"
```

### 시나리오 5: 복제 지연 분석 (MySQL)

```bash
# 1단계: 복제 상태 확인
whatap spot --pcode 12345 --keys replication_lag,replication_status

# 2단계: 복제 지연 추이
whatap stat query --pcode 12345 --category db_replica --field lag --duration 1h

# 3단계: 복제 관련 에러
whatap log search --pcode 12345 --category db_error_log --keyword "replication" --duration 1h
```

---

## 7. CI/CD 연동

### 배포 전 DB 상태 확인

```bash
#!/bin/bash
# pre_deployment_check.sh

PCODE=12345

echo "=== DB Health Check ==="

# 커넥션 풀 상태
echo "Connection Pool:"
whatap spot --pcode $PCODE --keys conn_pool_used,conn_pool_max

# 캐시 적중률
echo ""
echo "Cache Hit Ratio:"
whatap spot --pcode $PCODE --keys cache_hit_ratio

# 슬로우 쿼리 수
echo ""
echo "Slow Queries (>100ms):"
SLOW_COUNT=$(whatap mxql --pcode $PCODE "CATEGORY db_slow_query TAGLOAD SELECT [count] FILTER { avg_time > 100 } LIMIT 1" --json | jq '.[0].count // 0')
echo "Count: $SLOW_COUNT"

if [ "$SLOW_COUNT" -gt 50 ]; then
  echo "WARNING: Too many slow queries detected!"
fi
```

### 일일 DB 리포트

```bash
#!/bin/bash
# daily_db_report.sh

PCODE=12345
DATE=$(date +%Y%m%d)
OUTPUT_DIR="./reports/$DATE"

mkdir -p $OUTPUT_DIR

# TPS 통계
whatap stat query --pcode $PCODE --category db_counter --field tps --duration 24h --json > $OUTPUT_DIR/tps.json

# 응답시간 통계
whatap stat query --pcode $PCODE --category db_counter --field resp_time --duration 24h --json > $OUTPUT_DIR/resp_time.json

# 슬로우 쿼리
whatap mxql --pcode $PCODE "CATEGORY db_slow_query TAGLOAD SELECT [query_text, count, avg_time] FILTER { avg_time > 100 } LIMIT 50" --json > $OUTPUT_DIR/slow_queries.json

# 인덱스 사용률
whatap mxql --pcode $PCODE "CATEGORY db_index_usage TAGLOAD SELECT [index_name, table_name, access_count] LIMIT 100" --json > $OUTPUT_DIR/index_usage.json

# 에러 로그
whatap log search --pcode $PCODE --category db_error_log --level ERROR --duration 24h --json > $OUTPUT_DIR/errors.json

echo "DB Report generated: $OUTPUT_DIR"
```

---

## 8. 전체 명령어 옵션

### spot
```
--pcode <PCODE>      프로젝트 코드
--keys <KEYS>        특정 메트릭만 (쉼표 구분)
--json               JSON 출력
```

### stat query
```
--pcode <PCODE>      프로젝트 코드
--category <CAT>     메트릭 카테고리
--field <FIELD>      메트릭 필드
--duration <DUR>     시간 범위 (1h, 30m, 1d)
--json               JSON 출력
```

### mxql
```
--pcode <PCODE>      프로젝트 코드
--category <CAT>     카테고리만 지정
--stime <TIME>       시작 시간 (epoch ms)
--etime <TIME>       종료 시간 (epoch ms)
--limit <N>          최대 결과 수
--json               JSON 출력
```

### log search
```
--pcode <PCODE>      프로젝트 코드
-k, --keyword <KW>   검색 키워드
-l, --level <LEVEL>  로그 레벨 (ERROR, WARN, INFO)
--category <CAT>     로그 카테고리
--duration <DUR>     시간 범위
--limit <N>          최대 결과 수
--json               JSON 출력
```

---

## 9. 문제 해결

### 데이터가 안 보일 때

1. 시간 범위 확인 (`--duration`)
2. pcode 확인
3. DB 에이전트 실행 상태 확인
4. 권한 확인 (`whatap whoami`)

### 슬로우 쿼리 분석 팁

1. `EXPLAIN` 실행으로 실행 계획 확인
2. 인덱스 사용 여부 확인
3. 테이블 크기와 row 수 확인
4. 조건절 카디널리티 확인

### 성능 최적화 체크리스트

1. 캐시 적중률 95% 이상 유지
2. 미사용 인덱스 제거
3. 슬로우 쿼리 튜닝
4. 커넥션 풀 크기 적절히 설정
5. 정기적인 통계 갱신 (ANALYZE TABLE)

---

## 10. 자주 사용하는 카테고리

### DB 메트릭
- `db_counter` - TPS, 응답시간
- `db_pool` - 커넥션 풀 상태
- `db_cache` - 버퍼 캐시 적중률
- `db_lock` - 락 대기 통계
- `db_slow_query` - 슬로우 쿼리
- `db_query_stats` - 쿼리 통계
- `db_table_access` - 테이블 접근
- `db_index_usage` - 인덱스 사용률
- `db_sessions` - 세션 정보
- `db_replica` - 복제 상태

### DB 로그
- `db_error_log` - 에러 로그
- `db_slow_log` - 슬로우 쿼리 로그
- `db_general_log` - 일반 쿼리 로그
