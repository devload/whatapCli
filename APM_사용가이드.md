# WhatAp CLI - APM 서버 분석 예시

애플리케이션 성능 모니터링(APM) 데이터 분석 시나리오와 CLI 사용 예시입니다.

## 사전 요구사항

```bash
# 로그인 (이메일/비밀번호)
whatap login -e user@example.com -p 'password'

# 또는 API 키로 로그인 (CI/CD)
whatap login --api-key <key> --pcode <pcode>

# 프로젝트 확인
whatap projects --filter JAVA
whatap projects --filter NODE
whatap projects --filter PYTHON
```

---

## 1. 실시간 메트릭 조회 (Spot)

### 1.1 전체 메트릭 확인

```bash
# 모든 실시간 메트릭
whatap spot --pcode 12345
```

**출력 예시:**

```bash
$ whatap spot --pcode 12345
```
```
Active Transactions: 45
TPS: 128.5
Average Response Time: 245ms
CPU Usage: 67.2%
Memory Usage: 78.5%
Heap Used: 1.2GB / 2GB
GC Count: 15/min
SQL Count: 89/min
SQL Time: 120ms avg
Error Count: 3/min
```

### 1.2 특정 메트릭만 조회

```bash
# CPU와 TPS만
whatap spot --pcode 12345 --keys cpu,tps

# 응답시간과 에러
whatap spot --pcode 12345 --keys resp_time,err_count

# 활성 트랜잭션
whatap spot --pcode 12345 --keys actx
```

**출력 예시:**

```bash
$ whatap spot --pcode 12345 --keys cpu,tps,actx
```
```
cpu: 67.2%
tps: 128.5
actx: 45
```

### 1.3 JSON 출력

```bash
$ whatap spot --pcode 12345 --keys cpu,tps --json
```
```json
{
  "cpu": 67.2,
  "tps": 128.5,
  "timestamp": 1705312800000
}
```

---

## 2. 시계열 통계 조회 (Stat)

### 2.1 TPS 추이

```bash
# 지난 1시간 TPS
whatap stat query --pcode 12345 \
  --category app_counter \
  --field tps \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h
```
```
time                  value
2024-01-15 10:00      125.3
2024-01-15 10:05      132.1
2024-01-15 10:10      118.7
2024-01-15 10:15      145.2
2024-01-15 10:20      138.9
2024-01-15 10:25      142.5
```

### 2.2 응답시간 추이

```bash
# 지난 30분 평균 응답시간
whatap stat query --pcode 12345 \
  --category app_counter \
  --field resp_time \
  --duration 30m
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 30m
```
```
time                  value
2024-01-15 10:30      245ms
2024-01-15 10:35      312ms
2024-01-15 10:40      198ms
2024-01-15 10:45      267ms
2024-01-15 10:50      234ms
```

### 2.3 CPU/메모리 사용량

```bash
# CPU 사용률
whatap stat query --pcode 12345 \
  --category server_cpu \
  --field cpu \
  --duration 1h

# 메모리 사용률
whatap stat query --pcode 12345 \
  --category server_memory \
  --field mem \
  --duration 1h
```

**출력 예시:**

```bash
$ whatap stat query --pcode 12345 --category server_cpu --field cpu --duration 1h
```
```
time                  value
2024-01-15 10:00      45.2%
2024-01-15 10:10      52.8%
2024-01-15 10:20      78.3%
2024-01-15 10:30      65.1%
2024-01-15 10:40      71.4%
```

### 2.4 JSON 출력

```bash
$ whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h --json
```
```json
{
  "category": "app_counter",
  "field": "tps",
  "data": [
    {"time": 1705310400000, "value": 125.3},
    {"time": 1705310700000, "value": 132.1},
    {"time": 1705311000000, "value": 118.7}
  ]
}
```

---

## 3. 로그 검색 (Log)

### 3.1 기본 로그 검색

```bash
# 최근 로그
whatap log search --pcode 12345 --duration 10m

# 최근 1시간
whatap log search --pcode 12345 --duration 1h
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --duration 10m
```
```
50 log entries

[2024-01-15 10:45:23] INFO  [main] Application started successfully
[2024-01-15 10:46:12] DEBUG [http-nio-8080] Request received: GET /api/users
[2024-01-15 10:46:13] INFO  [http-nio-8080] Response sent: 200 OK (45ms)
[2024-01-15 10:47:01] WARN  [db-pool] Connection pool running low: 3/10 available
[2024-01-15 10:48:15] ERROR [http-nio-8080] Database connection failed: timeout
```

### 3.2 키워드 검색

```bash
# 에러 메시지 검색
whatap log search --pcode 12345 --keyword "error" --duration 1h

# 특정 예외 검색
whatap log search --pcode 12345 --keyword "NullPointerException" --duration 24h

# 특정 API 검색
whatap log search --pcode 12345 --keyword "/api/checkout" --duration 1h
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --keyword "error" --duration 1h
```
```
12 log entries matching "error"

[2024-01-15 10:15:23] ERROR [http-nio-8080] Database connection error: timeout
[2024-01-15 10:23:45] ERROR [http-nio-8080] Redis connection error: refused
[2024-01-15 10:31:12] ERROR [scheduler] Task execution error: NullPointerException
[2024-01-15 10:42:08] ERROR [http-nio-8080] API error: 500 Internal Server Error
```

### 3.3 로그 레벨 필터링

```bash
# 에러만
whatap log search --pcode 12345 --level ERROR --duration 1h

# 경고 이상
whatap log search --pcode 12345 --level WARN --duration 1h

# 디버그 로그
whatap log search --pcode 12345 --level DEBUG --duration 30m
```

**출력 예시:**

```bash
$ whatap log search --pcode 12345 --level ERROR --duration 1h
```
```
8 ERROR log entries

[2024-01-15 10:15:23] ERROR [http-nio-8080] Database connection failed
[2024-01-15 10:23:45] ERROR [http-nio-8080] Redis connection refused
[2024-01-15 10:31:12] ERROR [scheduler] Task execution failed
[2024-01-15 10:42:08] ERROR [http-nio-8080] API request failed
```

### 3.4 JSON 출력

```bash
$ whatap log search --pcode 12345 --level ERROR --duration 1h --json
```
```json
[
  {
    "timestamp": 1705311323000,
    "level": "ERROR",
    "thread": "http-nio-8080",
    "logger": "com.example.api.DatabaseHandler",
    "message": "Database connection failed: timeout after 30s"
  },
  {
    "timestamp": 1705311825000,
    "level": "ERROR",
    "thread": "http-nio-8080",
    "logger": "com.example.cache.RedisClient",
    "message": "Redis connection refused: localhost:6379"
  }
]
```

---

## 4. MXQL 직접 쿼리

### 4.1 애플리케이션 카운터

```bash
# TPS, 응답시간, 에러율
whatap mxql --pcode 12345 "CATEGORY app_counter
TAGLOAD
SELECT [tps, resp_time, err_rate, @timestamp]
LIMIT 100"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY app_counter TAGLOAD SELECT [tps, resp_time, err_rate, @timestamp] LIMIT 10"
```
```
tps     resp_time  err_rate  @timestamp
125.3   245ms      0.5%      2024-01-15 10:45:00
132.1   198ms      0.3%      2024-01-15 10:40:00
118.7   312ms      1.2%      2024-01-15 10:35:00
145.2   267ms      0.8%      2024-01-15 10:30:00
```

### 4.2 SQL 쿼리 분석

```bash
# 느린 SQL 쿼리
whatap mxql --pcode 12345 "CATEGORY sql_summary
TAGLOAD
SELECT [sql_text, count, avg_time, max_time]
FILTER { avg_time > 1000 }
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY sql_summary TAGLOAD SELECT [sql_text, count, avg_time, max_time] FILTER { avg_time > 1000 } LIMIT 10"
```
```
sql_text                                          count  avg_time  max_time
SELECT * FROM orders WHERE user_id = ?            1,234  1,520ms   5,230ms
SELECT * FROM products WHERE category = ?         892    1,890ms   4,120ms
UPDATE inventory SET stock = ? WHERE product_id = ? 567   1,340ms   3,450ms
SELECT COUNT(*) FROM audit_logs WHERE date > ?    234    2,450ms   8,920ms
```

### 4.3 트랜잭션 분석

```bash
# 느린 트랜잭션
whatap mxql --pcode 12345 "CATEGORY tx_detail
TAGLOAD
SELECT [service, method, count, avg_time, err_count]
FILTER { avg_time > 2000 }
LIMIT 50"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY tx_detail TAGLOAD SELECT [service, method, count, avg_time, err_count] FILTER { avg_time > 2000 } LIMIT 10"
```
```
service         method          count  avg_time  err_count
OrderService    checkout        234    3,450ms   12
PaymentService  processPayment  189    2,890ms   8
ReportService   generateReport  67     5,230ms   2
SearchService   fullTextSearch  456    2,120ms   3
```

### 4.4 HTTP 상태코드 분석

```bash
# 상태코드별 요청 수
whatap mxql --pcode 12345 "CATEGORY http_status
TAGLOAD
SELECT [status_code, count, @timestamp]
LIMIT 100"
```

**출력 예시:**

```bash
$ whatap mxql --pcode 12345 "CATEGORY http_status TAGLOAD SELECT [status_code, count] LIMIT 10"
```
```
status_code  count
200          45,678
201          1,234
301          567
400          234
401          89
404          156
500          45
502          12
503          8
```

---

## 5. 알림 관리 (Alert)

### 5.1 알림 목록 조회

```bash
# 전체 알림
whatap alert list --pcode 12345
```

**출력 예시:**

```bash
$ whatap alert list --pcode 12345
```
```
ID    Title                    Category      Status    Conditions
1001  High CPU Usage           server_cpu    enabled   cpu > 80% (warning), cpu > 95% (critical)
1002  High Response Time       app_counter   enabled   resp_time > 1000ms (warning)
1003  Error Rate Alert         app_counter   enabled   err_rate > 5% (critical)
1004  Memory Usage Warning     server_memory enabled   mem > 85% (warning)
1005  Low TPS Alert            app_counter   disabled  tps < 50 (warning)
```

### 5.2 알림 생성

```bash
# CPU 알림 생성
whatap alert create --pcode 12345 \
  --title "High CPU Usage" \
  --category server_cpu \
  --warning "cpu > 80" \
  --critical "cpu > 95" \
  --message "CPU usage is high: {{value}}%"

# 응답시간 알림 생성
whatap alert create --pcode 12345 \
  --title "High Response Time" \
  --category app_counter \
  --warning "resp_time > 2000" \
  --message "Response time exceeded: {{value}}ms"

# 에러율 알림 생성
whatap alert create --pcode 12345 \
  --title "Error Rate Alert" \
  --category app_counter \
  --critical "err_rate > 5" \
  --message "Error rate is too high: {{value}}%" \
  --repeat-count 3 \
  --repeat-duration 60
```

**출력 예시:**

```bash
$ whatap alert create --pcode 12345 --title "High CPU Usage" --category server_cpu --warning "cpu > 80" --critical "cpu > 95" --message "CPU usage is high: {{value}}%"
```
```
Alert created successfully
ID: 1001
Title: High CPU Usage
Category: server_cpu
Status: enabled
```

### 5.3 알림 활성화/비활성화

```bash
# 알림 비활성화
whatap alert disable --pcode 12345 --id 1001

# 알림 활성화
whatap alert enable --pcode 12345 --id 1001
```

**출력 예시:**

```bash
$ whatap alert disable --pcode 12345 --id 1001
```
```
Alert 1001 (High CPU Usage) disabled
```

### 5.4 알림 삭제

```bash
whatap alert delete --pcode 12345 --id 1001
```

**출력 예시:**

```bash
$ whatap alert delete --pcode 12345 --id 1001
```
```
Alert 1001 deleted
```

### 5.5 알림 내보내기/가져오기

```bash
# 내보내기
whatap alert export --pcode 12345 > alerts.json

# 가져오기
whatap alert import --pcode 12345 --file alerts.json
```

**출력 예시:**

```bash
$ whatap alert export --pcode 12345
```
```json
[
  {
    "id": 1001,
    "title": "High CPU Usage",
    "category": "server_cpu",
    "warning": "cpu > 80",
    "critical": "cpu > 95",
    "message": "CPU usage is high: {{value}}%",
    "enabled": true
  },
  {
    "id": 1002,
    "title": "High Response Time",
    "category": "app_counter",
    "warning": "resp_time > 1000",
    "message": "Response time exceeded: {{value}}ms",
    "enabled": true
  }
]
```

---

## 6. 분석 시나리오

### 시나리오 1: 장애 원인 분석

```bash
# 1단계: 실시간 상태 확인
whatap spot --pcode 12345

# 2단계: 에러 로그 확인
whatap log search --pcode 12345 --level ERROR --duration 1h

# 3단계: 응답시간 추이 확인
whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 1h

# 4단계: 느린 SQL 쿼리 확인
whatap mxql --pcode 12345 "CATEGORY sql_summary TAGLOAD SELECT [sql_text, avg_time, max_time] FILTER { avg_time > 1000 } LIMIT 20"

# 5단계: 느린 트랜잭션 확인
whatap mxql --pcode 12345 "CATEGORY tx_detail TAGLOAD SELECT [service, method, avg_time, err_count] FILTER { avg_time > 2000 } LIMIT 20"
```

### 시나리오 2: 성능 저하 분석

```bash
# 1단계: CPU/메모리 추이 확인
whatap stat query --pcode 12345 --category server_cpu --field cpu --duration 1h
whatap stat query --pcode 12345 --category server_memory --field mem --duration 1h

# 2단계: GC 빈도 확인 (Java)
whatap stat query --pcode 12345 --category jvm_gc --field gc_count --duration 1h

# 3단계: DB 커넥션 풀 확인
whatap mxql --pcode 12345 "CATEGORY db_pool TAGLOAD SELECT [active, idle, max] LIMIT 10"

# 4단계: 느린 API 엔드포인트 찾기
whatap mxql --pcode 12345 "CATEGORY api_performance TAGLOAD SELECT [endpoint, count, avg_time, p99] FILTER { avg_time > 500 } LIMIT 50"
```

### 시나리오 3: 배포 후 모니터링

```bash
# 1단계: 에러율 확인
whatap stat query --pcode 12345 --category app_counter --field err_rate --duration 30m

# 2단계: 새로운 에러 확인
whatap log search --pcode 12345 --level ERROR --duration 30m

# 3단계: TPS 변화 확인
whatap stat query --pcode 12345 --category app_counter --field tps --duration 1h

# 4단계: 응답시간 변화 확인
whatap stat query --pcode 12345 --category app_counter --field resp_time --duration 1h
```

### 시나리오 4: 일일 리포트 생성

```bash
#!/bin/bash
# daily_apm_report.sh

PCODE=12345
DATE=$(date +%Y%m%d)
OUTPUT_DIR="./reports/$DATE"

mkdir -p $OUTPUT_DIR

# TPS 통계
whatap stat query --pcode $PCODE --category app_counter --field tps --duration 24h --json > $OUTPUT_DIR/tps.json

# 응답시간 통계
whatap stat query --pcode $PCODE --category app_counter --field resp_time --duration 24h --json > $OUTPUT_DIR/resp_time.json

# 에러 로그
whatap log search --pcode $PCODE --level ERROR --duration 24h --json > $OUTPUT_DIR/errors.json

# 느린 SQL
whatap mxql --pcode $PCODE "CATEGORY sql_summary TAGLOAD SELECT [sql_text, count, avg_time] FILTER { avg_time > 500 } LIMIT 50" --json > $OUTPUT_DIR/slow_sql.json

echo "APM Report generated: $OUTPUT_DIR"
```

---

## 7. CI/CD 연동

### 배포 전후 비교

```bash
#!/bin/bash
# compare_metrics.sh

PCODE=12345
BEFORE=$(date -d "1 hour ago" +%s)000
AFTER=$(date +%s)000

echo "=== Before Deployment ==="
whatap spot --pcode $PCODE --keys tps,resp_time,err_rate

echo ""
echo "=== Checking for new errors ==="
whatap log search --pcode $PCODE --level ERROR --duration 1h --limit 5

echo ""
echo "=== Response time trend ==="
whatap stat query --pcode $PCODE --category app_counter --field resp_time --duration 2h
```

### 헬스체크 스크립트

```bash
#!/bin/bash
# health_check.sh

PCODE=12345
THRESHOLD_TPS=50
THRESHOLD_RESP_TIME=1000
THRESHOLD_ERR_RATE=5

# 실시간 메트릭 가져오기
METRICS=$(whatap spot --pcode $PCODE --keys tps,resp_time,err_rate --json)

TPS=$(echo $METRICS | jq -r '.tps')
RESP_TIME=$(echo $METRICS | jq -r '.resp_time')
ERR_RATE=$(echo $METRICS | jq -r '.err_rate')

ERRORS=""

if [ $(echo "$TPS < $THRESHOLD_TPS" | bc) -eq 1 ]; then
  ERRORS="$ERRORS\n- Low TPS: $TPS (threshold: $THRESHOLD_TPS)"
fi

if [ $(echo "$RESP_TIME > $THRESHOLD_RESP_TIME" | bc) -eq 1 ]; then
  ERRORS="$ERRORS\n- High Response Time: ${RESP_TIME}ms (threshold: ${THRESHOLD_RESP_TIME}ms)"
fi

if [ $(echo "$ERR_RATE > $THRESHOLD_ERR_RATE" | bc) -eq 1 ]; then
  ERRORS="$ERRORS\n- High Error Rate: ${ERR_RATE}% (threshold: ${THRESHOLD_ERR_RATE}%)"
fi

if [ -n "$ERRORS" ]; then
  echo "WARNING: Health check failed!"
  echo -e $ERRORS
  exit 1
else
  echo "OK: All metrics within threshold"
  echo "TPS: $TPS, Response Time: ${RESP_TIME}ms, Error Rate: ${ERR_RATE}%"
fi
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
--stime <TIME>       시작 시간 (epoch ms)
--etime <TIME>       종료 시간 (epoch ms)
--json               JSON 출력
```

### log search
```
--pcode <PCODE>      프로젝트 코드
-k, --keyword <KW>   검색 키워드
-l, --level <LEVEL>  로그 레벨 (ERROR, WARN, INFO, DEBUG)
--category <CAT>     로그 카테고리 (기본: app_log)
--fields <FIELDS>    커스텀 필드
--duration <DUR>     시간 범위
--limit <N>          최대 결과 수 (기본: 50)
--json               JSON 출력
```

### alert
```
alert list --pcode <PCODE>
alert create --pcode <PCODE> --title <TITLE> --category <CAT> [--warning <RULE>] [--critical <RULE>]
alert delete --pcode <PCODE> --id <ID>
alert enable --pcode <PCODE> --id <ID>
alert disable --pcode <PCODE> --id <ID>
alert export --pcode <PCODE>
alert import --pcode <PCODE> --file <FILE>
```

### mxql
```
--pcode <PCODE>      프로젝트 코드
--category <CAT>     카테고리만 지정 (자동 쿼리)
-f, --file <FILE>    MXQL 파일에서 읽기
--stime <TIME>       시작 시간 (epoch ms)
--etime <TIME>       종료 시간 (epoch ms)
--limit <N>          최대 결과 수
--json               JSON 출력
```

---

## 9. 문제 해결

### 데이터가 안 보일 때

1. 시간 범위 확인 (`--duration`, `--stime`, `--etime`)
2. pcode 확인
3. API 권한 확인 (`whatap whoami`)
4. 카테고리/필드명 확인

### 카테고리/필드 확인

```bash
# 사용 가능한 카테고리 목록
whatap stat categories --pcode 12345

# 로그 카테고리 목록
whatap log categories --pcode 12345
```

### MXQL 문법 에러

```bash
# verbose 모드로 쿼리 확인
whatap mxql --pcode 12345 "..." --verbose
```

---

## 10. 자주 사용하는 카테고리

### APM 메트릭
- `app_counter` - TPS, 응답시간, 에러율
- `tx_detail` - 트랜잭션 상세
- `sql_summary` - SQL 쿼리 요약
- `http_status` - HTTP 상태코드

### 시스템 메트릭
- `server_cpu` - CPU 사용률
- `server_memory` - 메모리 사용률
- `server_disk` - 디스크 사용률
- `server_network` - 네트워크 트래픽

### JVM (Java)
- `jvm_heap` - 힙 메모리
- `jvm_gc` - GC 통계
- `jvm_thread` - 스레드 상태

### DB
- `db_pool` - 커넥션 풀
- `db_query` - 쿼리 성능
