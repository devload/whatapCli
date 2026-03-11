# APM 서버 분석 가이드

이 문서는 애플리케이션 성능 모니터링(APM) 데이터를 분석하여 서버 장애, 성능 저하를 진단하고 해결하는 방법을 설명합니다.

---

## 1. 이상 징후 감지

### 1.1 모니터링해야 할 핵심 지표

| 지표 | 정상 범위 | 주의 | 심각 |
|------|----------|------|------|
| TPS (초당 트랜잭션) | 안정적 | ±30% 변동 | 급증/급감 |
| 평균 응답시간 | < 200ms | 200-500ms | > 500ms |
| P99 응답시간 | < 1s | 1-3s | > 3s |
| 에러율 | < 0.1% | 0.1-1% | > 1% |
| CPU 사용률 | < 60% | 60-80% | > 80% |
| 메모리 사용률 | < 70% | 70-85% | > 85% |
| 활성 스레드 | < 70% | 70-90% | > 90% |

### 1.2 이상 징후 패턴

**패턴 A: TPS는 정상인데 응답시간만 느림**
```
증상: TPS 500 → 500, 응답시간 50ms → 500ms
의심: DB 쿼리 느림, 외부 API 지연, GC 빈번
```

**패턴 B: TPS 급감 + 응답시간 급증**
```
증상: TPS 500 → 50, 응답시간 50ms → 5s
의심: 스레드 풀 고갈, DB 커넥션 풀 고갈, 데드락
```

**패턴 C: 에러율만 급증**
```
증상: 에러율 0.1% → 15%
의심: 외부 서비스 장애, 배포 후 회귀, 리소스 고갈
```

**패턴 D: CPU 100% 지속**
```
증상: CPU 30% → 100% 지속
의심: 무한루프, 연산 집약 작업, 스레드 누수
```

---

## 2. 원인 분석 플로우

### 2.1 응답시간 느림 → 원인 찾기

```
응답시간 느림
      │
      ▼
  어디서 느린가?
      │
  ┌───┼────────┐
  API  DB     외부
  │    │      │
  ▼    ▼      ▼
내부  쿼리   외부API
로직  느림   응답지연
  │
  ▼
CPU? 메모리?
```

**분석 명령어:**

```bash
# 1. 실시간 상태 확인
$ whatap spot --pcode 12345 --keys tps,resp_time,cpu,mem,err_rate

tps: 450
resp_time: 523ms      ← 느림
cpu: 45%              ← 정상
mem: 62%              ← 정상
err_rate: 0.3%        ← 정상

# 해석: CPU/메모리 정상인데 응답시간만 느림
# → I/O 바운드 문제 의심 (DB, 외부 API)
```

```bash
# 2. 응답시간 추이 확인
$ whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 1h

time                  value
2024-01-15 10:00      45ms
2024-01-15 10:10      52ms
2024-01-15 10:20      312ms   ← 문제 시작
2024-01-15 10:30      489ms
2024-01-15 10:40      523ms

# 해석: 10:20부터 응답시간 증가
# → 해당 시점에 무슨 일이 있었는지 확인 필요
```

```bash
# 3. 느린 SQL 쿼리 확인
$ whatap mxql --pcode 12345 "CATEGORY sql_summary
TAGLOAD
SELECT [sql_text, count, avg_time, max_time]
FILTER { avg_time > 100 }
LIMIT 10"

sql_text                                    count  avg_time  max_time
SELECT * FROM orders WHERE user_id = ?      1,234  320ms     1.2s
SELECT * FROM products WHERE category = ?   892    280ms     980ms
UPDATE inventory SET stock = ? WHERE ...    567    450ms     2.3s

# 해석: UPDATE inventory 쿼리가 450ms 소요
# → 재고 업데이트 로직 문제 의심
```

```bash
# 4. 느린 API 엔드포인트 확인
$ whatap mxql --pcode 12345 "CATEGORY tx_detail
TAGLOAD
SELECT [service, method, count, avg_time, err_count]
FILTER { avg_time > 200 }
LIMIT 10"

service         method          count  avg_time  err_count
OrderService    checkout        234    1.2s      12
PaymentService  processPayment  189    890ms     5
ReportService   generateReport  67     3.5s      0

# 해석: OrderService.checkout이 1.2초 소요
# → 결제 프로세스 병목
```

### 2.2 TPS 급감 → 원인 찾기

```
TPS 급감
    │
    ▼
스레드 상태?
    │
┌───┴────┐
BLOCKED  RUNNABLE
│        │
▼        ▼
락 대기  CPU 부족
│
├─ DB 락
├─ 동기화 블록
└─ 외부 리소스 대기
```

**분석 명령어:**

```bash
# 1. TPS 추이 확인
$ whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h

time                  value
2024-01-15 10:00      500
2024-01-15 10:10      480
2024-01-15 10:20      150    ← 급감
2024-01-15 10:30      50

# 해석: 10:20부터 TPS 급감
```

```bash
# 2. 활성 스레드 확인
$ whatap spot --pcode 12345 --keys active_threads,thread_pool_max

active_threads: 200
thread_pool_max: 200

# 해석: 스레드 풀 100% 사용 중
# → 요청 처리 불가 상태
```

```bash
# 3. 스레드 상태 분포
$ whatap mxql --pcode 12345 "CATEGORY jvm_thread
TAGLOAD
SELECT [threadState, count]
LIMIT 10"

threadState    count
RUNNABLE       45
BLOCKED        120    ← 문제
WAITING        35

# 해석: BLOCKED 상태 스레드 120개
# → 락 대기로 인한 병목
```

```bash
# 4. 락 대기 상세
$ whatap mxql --pcode 12345 "CATEGORY jvm_lock
TAGLOAD
SELECT [lockName, waitingThreads, ownerThread]
LIMIT 10"

lockName                    waitingThreads
OrderService.class          85
InventoryService.class      35

# 해석: OrderService의 클래스 락을 85개 스레드가 대기
# → synchronized 블록 병목
```

### 2.3 에러율 급증 → 원인 찾기

```
에러율 급증
    │
    ▼
에러 타입?
    │
┌───┼────┐
5xx 4xx  Timeout
│   │    │
▼   ▼    ▼
내부 권한 외부/지연
장애 문제
```

**분석 명령어:**

```bash
# 1. 에러율 추이
$ whatap stat query --pcode 12345 --category app_counter --field err_rate --duration 1h

time                  value
2024-01-15 10:00      0.1%
2024-01-15 10:10      0.2%
2024-01-15 10:20      5.5%   ← 문제 시작
2024-01-15 10:30      12.3%

# Step 2: 에러 로그 확인
$ whatap log search --pcode 12345 --level ERROR --duration 30m

[2024-01-15 10:20:15] ERROR Database connection failed: timeout
[2024-01-15 10:20:18] ERROR Database connection failed: timeout
[2024-01-15 10:20:22] ERROR Database connection failed: timeout
...

# 해석: DB 연결 타임아웃 반복
# → DB 장애 또는 커넥션 풀 고갈
```

```bash
# 3. DB 커넥션 풀 상태
$ whatap spot --pcode 12345 --keys db_pool_active,db_pool_max

db_pool_active: 50
db_pool_max: 50

# 해석: 커넥션 풀 100% 사용 중
# → 새 요청이 커넥션 대기하다 타임아웃
```

### 2.4 CPU 100% → 원인 찾기

```
CPU 100%
    │
    ▼
어떤 스레드가?
    │
┌───┴────┐
단일     다수
│        │
▼        ▼
루프/버그 정상부하
```

**분석 명령어:**

```bash
# 1. CPU 추이
$ whatap stat query --pcode 12345 --category server_cpu --field cpu --duration 1h

time                  value
2024-01-15 10:00      35%
2024-01-15 10:10      42%
2024-01-15 10:20      95%    ← 문제
2024-01-15 10:30      100%

# Step 2: CPU 많이 쓰는 스레드
$ whatap mxql --pcode 12345 "CATEGORY jvm_thread_cpu
TAGLOAD
SELECT [threadName, cpuTime, state]
FILTER { cpuTime > 1000 }
LIMIT 10"

threadName              cpuTime    state
scheduler-worker-1      45,230ms   RUNNABLE
http-nio-8080-exec-15   12,345ms   RUNNABLE
http-nio-8080-exec-23   11,890ms   RUNNABLE

# 해석: scheduler-worker-1이 CPU 45초 사용
# → 스케줄러 작업 문제 의심
```

```bash
# 3. 해당 스레드 스택트레이스
$ whatap mxql --pcode 12345 "CATEGORY jvm_thread_dump
TAGLOAD
SELECT [threadName, stacktrace]
FILTER { threadName == 'scheduler-worker-1' }
LIMIT 1"

threadName: scheduler-worker-1
stacktrace:
  at com.app.service.ReportService.generateMonthlyReport()
  at com.app.scheduler.ReportScheduler.run()
  ...

# 해석: 월간 리포트 생성이 CPU 과다 사용
# → 대량 데이터 처리 최적화 필요
```

---

## 3. 실제 트러블슈팅 케이스

### 케이스 1: 매일 오후 2시 서버 응답 지연

**증상:**
- 매일 오후 2시경 응답시간 50ms → 3초
- 30분 후 자동 회복

**분석 과정:**

```bash
# Step 1: 시간대별 응답시간 패턴
$ whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 7d

# 매일 14:00-14:30 응답시간 스파이크 확인

# Step 2: 해당 시간대 DB 쿼리
$ whatap mxql --pcode 12345 "CATEGORY sql_summary
TAGLOAD
SELECT [sql_text, count, avg_time]
FILTER { @timestamp >= 1705314000000 AND @timestamp < 1705315800000 }
LIMIT 10"

sql_text                                    count  avg_time
DELETE FROM user_sessions WHERE expires...  1      45,000ms  ← 여기!
SELECT * FROM orders WHERE ...              5,678  45ms

# Step 3: 세션 정리 쿼리 분석
# → 180만 건 세션 데이터를 단일 DELETE로 처리
```

**원인:** 세션 정리 배치가 180만 건을 한 번에 DELETE

**해결:**
```sql
-- Before (45초)
DELETE FROM user_sessions WHERE expires_at < NOW();

-- After (배치 처리)
DELETE FROM user_sessions WHERE expires_at < NOW() LIMIT 10000;
-- 반복 실행
```

---

### 케이스 2: 결제 API 타임아웃 반복

**증상:**
- 결제 API 5초 타임아웃 에러 반복
- 사용자 결제 실패 CS 증가

**분석 과정:**

```bash
# Step 1: 에러 로그
$ whatap log search --pcode 12345 --keyword "timeout" --level ERROR --duration 1h

[2024-01-15 10:15:23] ERROR PaymentService: Connection timeout to PG server
[2024-01-15 10:15:45] ERROR PaymentService: Connection timeout to PG server

# Step 2: 결제 API 응답시간
$ whatap mxql --pcode 12345 "CATEGORY tx_detail
TAGLOAD
SELECT [method, avg_time, err_count]
FILTER { service == 'PaymentService' }
LIMIT 10"

method          avg_time  err_count
processPayment  5.2s      234       ← 타임아웃
verifyPayment   3.8s      89

# Step 3: 외부 API 호출 추적
$ whatap mxql --pcode 12345 "CATEGORY external_api
TAGLOAD
SELECT [url, count, avg_time, err_count]
LIMIT 10"

url                           count  avg_time  err_count
https://pg.example.com/pay    1,234  4.8s      234

# 해석: PG사 API 응답이 평균 4.8초, 타임아웃 234건
# → PG사 측 문제 또는 네트워크 문제
```

**원인:** PG사 서버 응답 지연

**해결:**
1. PG사 장애 공지 확인 → PG사 장애
2. 대체 PG사로 라우팅
3. 타임아웃 단축 + 재시도 로직 추가

---

### 케이스 3: 메모리 누수로 인한 OOM

**증상:**
- 주기적으로 애플리케이션 재시작
- 로그에 OutOfMemoryError

**분석 과정:**

```bash
# Step 1: 메모리 추이
$ whatap stat query --pcode 12345 --category jvm_heap --field used --duration 24h

time                  value
2024-01-15 00:00      512MB   ← 재시작 후
2024-01-15 06:00      890MB
2024-01-15 12:00      1.2GB
2024-01-15 14:30      1.8GB   ← OOM

# 해석: 메모리가 지속적으로 증가 → 메모리 누수

# Step 2: GC 빈도 확인
$ whatap stat query --pcode 12345 --category jvm_gc --field gc_count --duration 24h

time                  value
2024-01-15 00:00      5/min
2024-01-15 12:00      45/min
2024-01-15 14:30      120/min  ← Full GC 빈번

# Step 3: 객체 수 확인
$ whatap mxql --pcode 12345 "CATEGORY jvm_object
TAGLOAD
SELECT [className, instanceCount, size]
LIMIT 10"

className                instanceCount  size
com.app.cache.UserCache  1,234,567      890MB  ← 누수 의심
com.app.model.Order      89,234         45MB
```

**원인:** UserCache에서 사용 완료 객체를 제거하지 않음

**해결:**
```java
// Before (누수)
public class UserCache {
    private Map<String, User> cache = new HashMap<>();

    public void put(User user) {
        cache.put(user.getId(), user);
        // 제거 로직 없음
    }
}

// After (WeakHashMap 또는 LRU)
public class UserCache {
    private Map<String, User> cache = new WeakHashMap<>();
    // 또는
    private Cache<String, User> cache = Caffeine.newBuilder()
        .maximumSize(10000)
        .expireAfterWrite(10, TimeUnit.MINUTES)
        .build();
}
```

---

## 4. 핵심 분석 체크리스트

### 응답시간 느릴 때

- [ ] CPU 정상인가? → I/O 바운드 의심
- [ ] DB 쿼리 느린가? → SQL 튜닝
- [ ] 외부 API 느린가? → 타임아웃/캐시
- [ ] GC 빈번한가? → 메모리 최적화
- [ ] 특정 API만 느린가? → 해당 기능 집중 분석

### TPS 급감 시

- [ ] 스레드 풀 포화? → 스레드 수 증설
- [ ] DB 커넥션 풀 포화? → 커넥션 수 증설
- [ ] 락 대기? → 동시성 제어 개선
- [ ] 외부 서비스 대기? → 비동기/타임아웃

### 에러율 급증 시

- [ ] 5xx 에러? → 서버 내부 문제
- [ ] DB 연결 실패? → 커넥션 풀/DB 상태
- [ ] 타임아웃? → 외부 서비스/네트워크
- [ ] 특정 에러 패턴? → 로그 상세 분석

### CPU 100% 시

- [ ] 특정 스레드 과다? → 해당 로직 최적화
- [ ] GC 과다? → 메모리 누수
- [ ] 요청 급증? → 스케일 아웃
- [ ] 무한루프? → 코드 버그

---

## 5. 빠른 진단 명령어

```bash
# 실시간 상태 (가장 먼저 확인)
$ whatap spot --pcode 12345

# TPS/응답시간/에러율 추이
$ whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h
$ whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 1h
$ whatap stat query --pcode 12345 --category app_counter --field err_rate --duration 1h

# 최근 에러 로그
$ whatap log search --pcode 12345 --level ERROR --duration 30m

# 느린 SQL
$ whatap mxql --pcode 12345 "CATEGORY sql_summary TAGLOAD SELECT [sql_text, avg_time] FILTER { avg_time > 100 } LIMIT 10"

# 느린 API
$ whatap mxql --pcode 12345 "CATEGORY tx_detail TAGLOAD SELECT [service, method, avg_time] FILTER { avg_time > 500 } LIMIT 10"
```
