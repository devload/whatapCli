# whatap-cli

Rust 기반 WhatAp 모니터링 플랫폼 CLI 도구.

## 빌드 및 실행

```bash
cargo build --release           # 릴리즈 빌드 → target/release/whatap
cargo run -- <command>          # 개발 실행
cargo test                      # 테스트
```

## 프로젝트 구조

```
src/
├── main.rs           # CLI 진입점, 명령어 정의 (clap)
├── core/
│   ├── auth.rs       # 인증 (이메일/API키)
│   ├── client.rs     # HTTP 클라이언트, API 호출
│   ├── config.rs     # 설정 해상도 (profile/env/file)
│   ├── error.rs      # 커스텀 에러 타입
│   └── symbol.rs     # 심볼 업로드 로직
├── types/
│   ├── auth.rs       # Credentials, SessionData
│   ├── config.rs     # ResolvedConfig
│   ├── project.rs    # Project 구조체
│   └── symbol.rs     # 심볼 관련 타입
└── cli/
    ├── mod.rs
    ├── output.rs     # 출력 포맷팅 (table/json)
    └── commands/     # 각 명령어 구현
        ├── login.rs, logout.rs, whoami.rs
        ├── projects.rs, project.rs, info.rs
        ├── mxql.rs
        ├── spot.rs, stat.rs, log.rs
        ├── step.rs   # Browser RUM (resources/ajax/errors/pageload)
        ├── trace.rs  # Browser 데이터 연관 분석
        ├── alert.rs
        └── sourcemaps.rs, proguard.rs, dsym.rs
```

## 인증 방식

### 1. 이메일/비밀번호 로그인
```bash
whatap login -e user@example.com -p 'password'
```
- 모바일 API로 로그인 → API 토큰 획득
- 웹 로그인 → JSESSIONID + wa 쿠키 획득 (MXQL/Step 사용)

### 2. API 키 로그인
```bash
whatap login --api-key <key> --pcode <pcode>
```
- 프로젝트 스코프 제한
- MXQL/Step 사용 불가 (웹 세션 필요)

## 주요 명령어

### 인증
```bash
whatap login -e <email> -p <password>
whatap login --api-key <key> --pcode <pcode>
whatap logout [--all]
whatap whoami
```

### 프로젝트
```bash
whatap projects [--filter MOBILE|BROWSER|APM]
whatap project create --name <n> --platform <p>
whatap project delete <pcode>
whatap info <pcode>
```

### MXQL 쿼리
```bash
whatap mxql --pcode 123 "CATEGORY app_counter\nTAGLOAD\nSELECT"
whatap mxql --pcode 123 --category app_counter
whatap mxql --pcode 123 -f query.mxql
whatap mxql --json --input-json '{"pcode":123,"mql":"...","stime":...,"etime":...}'
```

### 메트릭 (Stat)
```bash
whatap stat query --category app_counter --field tps --duration 1h
whatap stat categories
```

### 실시간 메트릭 (Spot)
```bash
whatap spot --pcode 123 --keys cpu,tps,actx
```

### 로그
```bash
whatap log search --keyword error --level ERROR --duration 30m
whatap log categories
```

### Browser RUM (Step)
```bash
whatap step resources --pcode 123 --type script --slow 1000
whatap step ajax --pcode 123 --errors --slow 500
whatap step errors --pcode 123 --type TypeError
whatap step pageload --pcode 123 --slow 3000
```

### Browser 데이터 연관 분석 (Trace)
```bash
# step 명령으로 Key 확인
whatap step pageload --pcode 123 --duration 1h
# 출력 예: /cart@473000 (count: 5, total: 3.50s)

# 해당 페이지의 모든 연관 데이터 조회
whatap trace /cart@473000 --pcode 123

# JSON 출력으로 상세 분석
whatap trace /cart@473000 --pcode 123 --raw --json
```

`trace` 명령은 특정 페이지(page_group)에 대한 모든 연관 데이터를 조회합니다:
- Page Load: 페이지 로드 타이밍 (TTFB, Backend, Frontend, Render)
- AJAX Requests: API 호출 목록 (URL, 횟수, 소요시간, 에러)
- Resources: 로드된 리소스 (타입, URL, 크기, 소요시간)
- Errors: 발생한 에러 (타입, 메시지, 브라우저, 기기)

### 알릿
```bash
whatap alert list --pcode 123
whatap alert create --pcode 123 --title "High CPU" --category app_counter --warning "cpu > 80"
whatap alert enable/disable --pcode 123 --id <id>
whatap alert export/import --pcode 123 -f alerts.json
```

### 심볼 업로드
```bash
whatap sourcemaps upload ./dist --pcode 123 --version 1.0.0
whatap proguard upload ./mapping.txt --pcode 123
whatap dsym upload ./App.dSYM --pcode 123
```

## 글로벌 옵션

```
--json         JSON 출력
--quiet        불필요한 출력 제거
--verbose      상세 로깅 (MXQL 쿼리 등)
--profile      인증 프로필 (기본: default)
--server       서버 URL 오버라이드
--no-color     색상 비활성화
```

## 종료 코드

| 코드 | 의미 |
|-----|------|
| 0 | 성공 |
| 2 | 인증 에러 |
| 3 | 설정 에러 |
| 4 | API 에러 |
| 6 | 입력 에러 |
| 1 | 기타 에러 |

## 데이터 연관관계

WhatAp는 여러 데이터 타입을 수집하며, 각 데이터는 공통 식별자를 통해 연결됩니다.

### 데이터 타입별 연결 키

| 데이터 타입 | 명령어 | 카테고리 | 주요 연결 키 |
|------------|--------|---------|-------------|
| Browser 리소스 | `step resources` | `rum_resource_each_page` | `page_group`, `@timestamp` |
| Browser AJAX | `step ajax` | `rum_ajax_each_page` | `page_group`, `@timestamp` |
| Browser 에러 | `step errors` | `rum_error_total_each_page` | `page_group`, `error_type`, `@timestamp` |
| 페이지 로드 | `step pageload` | `rum_page_load_each_page` | `page_group`, `@timestamp` |
| 앱 로그 | `log search` | `app_log` | `oid`, `oname`, `@timestamp` |
| 서버 로그 | `log search` | `server_log` | `oid`, `oname`, `@timestamp` |
| 앱 메트릭 | `stat query` | `app_counter` | `oid`, `oname`, `time` |
| 서버 메트릭 | `stat query` | `server_cpu`, `server_memory` | `oid`, `oname`, `time` |
| 실시간 카운터 | `spot` | - | `pcode` |

### 공통 식별자

```
┌─────────────────────────────────────────────────────────────────┐
│  pcode (프로젝트 코드)                                           │
│  └── 모든 데이터의 최상위 분류                                    │
│                                                                  │
│  @timestamp (에포크 밀리초)                                       │
│  └── 시간 기준 데이터 연결                                        │
│                                                                  │
│  page_group (Browser RUM)                                        │
│  └── 페이지 URL 기반 - step 데이터 간 연결                        │
│                                                                  │
│  oid / oname (APM)                                               │
│  └── 에이전트 식별 - 로그/메트릭 간 연결                          │
│                                                                  │
│  session (Mobile/RUM)                                            │
│  └── 사용자 세션 기반 추적                                        │
└─────────────────────────────────────────────────────────────────┘
```

### 연관 분석 시나리오

#### 1. Browser 에러 원인 분석
```
에러 발생 → step errors 조회
    ↓
같은 page_group, 시간대의 AJAX 오류? → step ajax --errors
    ↓
같은 시간대 느린 리소스? → step resources --slow
    ↓
페이지 로드 타이밍 이상? → step pageload --slow
```

```bash
# 예시: 특정 페이지 에러 분석
whatap step errors --pcode 123 --page "/dashboard" --duration 1h
whatap step ajax --pcode 123 --page "/dashboard" --errors --duration 1h
whatap step resources --pcode 123 --page "/dashboard" --slow 2000 --duration 1h
```

#### 2. APM 성능 이슈 추적
```
응답시간 증가 → stat query app_counter/resp_time
    ↓
같은 에이전트의 에러 로그? → log search --level ERROR
    ↓
DB 쿼리 문제? → stat query app_sql/time
    ↓
외부 호출 문제? → stat query app_httpc/time
```

```bash
# 예시: 특정 에이전트 성능 분석
whatap stat query --category app_counter --field resp_time --duration 1h
whatap log search --level ERROR --duration 1h
whatap stat query --category app_sql --field time --duration 1h
```

#### 3. 전체 시스템 상태 파악
```
페이지 로드 지연 → step pageload
    ↓
백엔드 응답 지연? → stat query app_counter/resp_time
    ↓
서버 리소스 부족? → stat query server_cpu, server_memory
    ↓
DB 커넥션 풀 고갈? → stat query db_pool
```

### MXQL 카테고리 참조

#### Browser RUM
| 카테고리 | 설명 | 주요 필드 |
|---------|------|----------|
| `rum_resource_each_page` | 리소스 로딩 | `page_group`, `resource_url`, `resource_type`, `duration`, `size`, `status` |
| `rum_ajax_each_page` | AJAX 요청 | `page_group`, `ajax_url`, `ajax_method`, `ajax_time`, `ajax_status`, `ajax_error_rate` |
| `rum_error_total_each_page` | JS 에러 | `page_group`, `error_type`, `error_message`, `count`, `browser`, `device` |
| `rum_page_load_each_page` | 페이지 로드 | `page_group`, `pageLoadTime`, `ttfb`, `backendTime`, `frontendTime`, `renderTime` |
| `rum_web_vitals_each_page` | Core Web Vitals | `page_group`, `lcp`, `fid`, `cls` |

#### APM
| 카테고리 | 설명 | 주요 필드 |
|---------|------|----------|
| `app_counter` | 앱 카운터 | `tps`, `resp_time`, `actx`, `apdex`, `error_cnt` |
| `app_sql` | SQL 실행 | `count`, `error`, `time`, `fetch` |
| `app_httpc` | HTTP 아웃바운드 | `count`, `error`, `time` |
| `app_user` | 실시간 사용자 | `realtime_user` |

#### 인프라
| 카테고리 | 설명 | 주요 필드 |
|---------|------|----------|
| `server_cpu` | CPU | `cpu`, `load1`, `load5`, `load15` |
| `server_memory` | 메모리 | `memory_pused`, `memory_available` |
| `server_disk` | 디스크 | `disk_usage`, `disk_io` |
| `server_network` | 네트워크 | `traffic_in`, `traffic_out` |
| `db_pool` | DB 커넥션 | `active_connection`, `idle_connection` |

#### Mobile
| 카테고리 | 설명 | 주요 필드 |
|---------|------|----------|
| `mobile_crash` | 크래시 | `crashType`, `crashMessage`, `device`, `osVersion` |
| `mobile_exception` | 예외 | `exceptionType`, `exceptionMessage` |
| `mobile_device_session` | 세션 | `session_count`, `crash_count`, `anr_count` |

## 설정 파일

### 자격증명
```
~/.whatap/credentials/<profile>.json
```

### 프로젝트 설정 (.whataprc.yml)
```yaml
pcode: 12345
server: https://service.whatap.io
```

## 코드 컨벤션

- Rust 2021 edition
- clap derive 매크로로 CLI 정의
- anyhow/thiserror로 에러 처리
- tabled로 테이블 출력
- serde_json으로 JSON 처리
- 비동기는 tokio 사용

### 새 명령어 추가

1. `src/cli/commands/`에 새 파일 생성
2. `src/cli/commands/mod.rs`에 모듈 등록
3. `src/main.rs`의 Commands enum에 서브커맨드 추가
4. main.rs의 match 블록에 핸들러 연결

### MXQL 쿼리 빌드 패턴

```rust
// 기본 구조
let mql = format!(
    "CATEGORY {}\nTAGLOAD\nSELECT [{}]\nFILTER {{ {} }}\nLIMIT {}",
    category, fields, filters.join(" && "), limit
);

// yard API 요청
let request = serde_json::json!({
    "type": "mxql",
    "pcode": pcode,
    "params": { "pcode": pcode, "stime": stime, "etime": etime, "mql": mql, ... },
    "path": "text",
    "authKey": ""
});
let result = client.yard_post(&request).await?;
```
