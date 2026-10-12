# Vutnet — Agent Instructions

## 1. Project Overview

`vutnet` là web framework native dành cho ngôn ngữ Vut.

Mục tiêu của Vutnet là cung cấp developer experience đơn giản tương tự các framework như Express, nhưng API phải được thiết kế theo đặc trưng riêng của Vut:

- static typing;
- Receiver Functions;
- typed callbacks;
- VPM packages;
- VPM local packages;
- native packages;
- Vut syntax ở application layer;
- Rust ở system/runtime layer.

Target user experience:

```vut
import vutnet
import vutnet.router as router

fn main() -> int:
    app = vutnet.app()
    app.router(router.RootRouter)
    app.listen(3000)
    0
```

This application shape is implemented and covered by the real-socket Phase 3
test in `crates/vpm/tests/vutnet_http_e2e.rs`. Vut executable examples must
place startup calls inside `fn main() -> int`.

`App.with_max_body_bytes` configures the bounded request body without exposing
server implementation types. `App.listen` blocks and drains active connections
on Ctrl+C, Unix SIGTERM or Windows Ctrl+Break; Vutnet does not expose a
background server handle or programmatic stop method.

`vutnet.response` provides `send`, `send_bytes`, `html`, `json(json.Value)`,
`json_encodable(json.Encodable)`, overloaded typed `render(Page)` and
`render(Page, page_data)`, opt-in typed `render_hydratable(Page[, page_data])`,
and immutable `Response.with_header`. The compiler synthesizes `to_json()` for
eligible non-generic `data` records with supported JSON fields, so ordinary
hydratable page data needs no user-authored codecs; custom `to_json()` remains
available for deliberate wire formats. The response emits the versioned bootstrap and shared plan.
Browser `hydrateView(container, Page)` validates that plan and attaches events
and state subscriptions to existing DOM without replacing it initially; later
writes evaluate only subscribed typed bindings and update their nodes/regions.
Eager reads outside bindings retain full-View compatibility refreshes.
Unique HTML IDs preserve reorderable list/component roots; other nodes use
site/local sibling occurrence under retained parents. Inputs preserve user
edits unless declarative values change. Scroll/focus follow retained nodes.
Each VPM consumer project automatically gets a generated typed client registry
from its checked calls to `response.render_hydratable(Page[, data])`. VPM
generates one statically typed browser factory per Page call-site; ordinary
nominal page data uses the compiler-generated JSON schema, while a public static
`Data.from_json` is automatically wrapped for custom decoding. No consumer
`[browser]` config, client `[[bin]]`, factory, decoder or generated source file
is required. `vutnet dev`, `run` and `build` invoke the VPM hydration-registry
build and stage Wasm, JS bindings, registry and bootstrap assets. The browser
bootstrap selects the Page by the versioned SSR call-site ID and calls the
existing `hydrateView(container, Page)` matcher. Dev rebuilds retain last-known-
good server/client generations; `start` serves a validated packaged bundle
without invoking the compiler. Normal `response.render` stays SSR-only.
Hydratable pages enhance eligible same-origin links and GET/POST forms through
the existing server Router and SSR bootstrap, support Back/Forward, and fall back
to normal browser navigation for unsupported responses. POST handlers should use
303 Post/Redirect/Get. See `specs/15-client-navigation.md`.
`View` is the
public type; `view(...)` builds a View, layout functions also return ordinary
Views, and Page-owned slots resolve during `view(layout = Layout):` before the
HTML5ever-backed low-level `vutnet.view.render`. A slot without a body is
required; a slot with a body has fallback content. Slot supply and resolution
types are private. See `specs/10-view-system.md`. Keep model serialization
compile-time typed, document the eligibility limits, and do not introduce runtime reflection.

Middleware is implemented with typed callable aliases:

```vut
type Next = fn(Context) -> Response
type Middleware = fn(Context, Next) -> Response

app.use(Logger)              # global middleware
Use(Auth)                    # Router/Routes scope middleware
UseRouter(path = "/api", router = ApiRouter) # RouterFactory mount
```

`Use` and `UseRouter` have distinct meanings. Never use the old
`Use(path = ..., router = ...)` mount form. Middleware execution and callback
limits are documented in `specs/04-middleware.md`.

Router sử dụng Receiver Functions:

```vut
import vutnet.router at router, Router

fn RootRouter() -> Router:
    router():
        Route(path = "/", controller = HomeController)
        Routes(path = "/admin"):
            Route(path = "/", controller = AdminController)
            Route(path = "/setting", controller = SettingController)
```

Vutnet không phải pure-Vut framework.

Kiến trúc chủ đạo:

```text
Vut application/API
        ↓
Vutnet Vut layer
        ↓
Native boundary
        ↓
Rust implementation
        ↓
mature Rust crates
        ↓
Operating System
```

Vut chủ yếu biểu diễn public API, types, routing DSL và application logic.

Rust đảm nhiệm các phần system-level và performance-sensitive như:

- networking;
- HTTP;
- async runtime;
- sockets;
- connection lifecycle;
- buffers;
- concurrency;
- native resource management.

The official `vutnet` command is owned by this VPM package. Declare it with
`[[bin]]` pointing to `src/bin/vutnet.vut`; keep command selection and argument
validation in `src/cli.vut`. Put OS/process/filesystem work in
`native/src/cli/` and expose it through typed `extern "VutManaged"` imports
backed by `vut-native-sdk`. Keep this Rust source inside the native build tree
so VPM's native source fingerprint includes it. Do not add the CLI to Vut core,
create a separate Rust executable, or add a VPM/compiler special case.

---

# 2. Source of Truth Priority

Các tài liệu trong:

```text
vutnet/specs/
```

là design documents và định hướng implementation.

**Specs KHÔNG phải nguồn sự thật tuyệt đối.**

Specs có thể:

- cũ;
- thiếu;
- sai;
- không còn phù hợp với Vut hiện tại;
- mô tả API/compiler/runtime đã thay đổi;
- mâu thuẫn với root Vut specifications;
- mâu thuẫn với implementation thực tế.

Khi có mâu thuẫn, ưu tiên theo thứ tự:

```text
1. Root Vut specs / language specifications
2. Vut/VPM/compiler/runtime source code hiện tại
3. Tests hiện tại của Vut/VPM
4. Existing public behavior
5. vutnet/specs/
```

Không được ép code hiện tại phải tuân theo một assumption sai trong `vutnet/specs`.

Trước khi implement feature phụ thuộc vào behavior của Vut, VPM, FFI, native packages, callbacks, Receiver Functions hoặc ownership:

**hãy đọc implementation/spec tương ứng của Vut trước.**

Không đoán behavior.

---

# 3. Specs May Be Corrected

Agent được phép sửa:

```text
vutnet/specs/*.md
```

khi phát hiện spec:

- sai với Vut hiện tại;
- không thể implement theo language semantics;
- mâu thuẫn với root specs;
- mâu thuẫn với architecture đã được chứng minh bằng code;
- dùng API đã thay đổi;
- có assumption kỹ thuật sai.

Khi sửa spec:

1. xác định lý do;
2. ưu tiên behavior thực tế của Vut;
3. giữ nguyên mục tiêu cấp cao của Vutnet nếu có thể;
4. cập nhật spec để phản ánh architecture mới;
5. ghi rõ thay đổi trong báo cáo cuối phase.

Không được âm thầm thay đổi product direction chỉ vì implementation khác spec.

Nếu thay đổi ảnh hưởng lớn tới public API của Vutnet, hãy dừng và báo trước.

---

# 4. Prefer Existing Rust Crates

Đây là rule quan trọng.

**Ưu tiên sử dụng Rust crates trưởng thành thay vì tự implement lại functionality đã tồn tại.**

Ví dụ networking/web stack có thể khảo sát:

```text
tokio
hyper
http
http-body-util
bytes
```

Tương lai có thể dùng:

```text
rustls
tokio-rustls
tokio-tungstenite
```

hoặc crate khác nếu phù hợp hơn.

Không giới hạn ở danh sách trên.

Agent được phép tự khảo sát crates phù hợp với từng requirement.

---

# 5. Do Not Reinvent Infrastructure

Không tự viết lại những thành phần phức tạp đã có implementation Rust tốt, trừ khi có lý do kỹ thuật rõ ràng.

Ví dụ, mặc định KHÔNG tự viết:

```text
HTTP parser
TCP async runtime
async executor
TLS implementation
WebSocket protocol
URL parser
header parser
multipart parser
compression algorithms
cryptography
```

nếu ecosystem Rust đã có crate phù hợp.

Vutnet nên tập trung vào:

```text
Vut API
Vut DSL
FFI/native bridge
framework semantics
routing
middleware
request/response abstraction
developer experience
```

thay vì tái phát minh infrastructure.

---

# 6. Installing Dependencies Is Allowed

Agent **được phép cài/thêm dependencies cần thiết**.

Bao gồm:

- Cargo dependencies;
- development dependencies;
- VPM dependencies;
- workspace dependencies;
- test utilities;
- build dependencies.

Không cần tránh dependency chỉ để giữ dependency count thấp.

Tuy nhiên mỗi dependency phải có mục đích rõ ràng.

Trước khi thêm dependency lớn:

- kiểm tra project đã có dependency tương đương chưa;
- tránh duplicate functionality;
- ưu tiên crate maintained và phổ biến;
- cân nhắc native build impact;
- tránh framework cấp cao nếu nó ép architecture của Vutnet đi theo framework đó.

Ví dụ:

Nếu chỉ cần HTTP engine thì:

```text
Hyper + Tokio
```

có thể phù hợp hơn việc đặt toàn bộ Vutnet trên Axum.

Nhưng đây không phải rule cứng.

Nếu code thực tế chứng minh crate khác phù hợp hơn, được phép chọn crate đó.

---

# 7. Modular Architecture Is Required

Agent **được phép và được khuyến khích tạo file/folder mới**.

Điều này áp dụng cho cả:

```text
Vut
Rust
tests
examples
build tooling
native bridge
```

Không nhét nhiều unrelated logic vào một file lớn.

Nếu module bắt đầu chứa nhiều responsibility, hãy chia nó.

Ví dụ Vut side có thể phát triển thành:

```text
src/
├── mod.vut
├── app.vut
├── request.vut
├── response.vut
├── middleware.vut
└── router/
    ├── mod.vut
    ├── router.vut
    ├── routes.vut
    ├── route.vut
    └── definitions.vut
```

Rust side cũng phải modular tương tự.

Ví dụ:

```text
native/
└── crates/
    ├── vutnet-native/
    │   └── src/
    │       ├── lib.rs
    │       ├── server.rs
    │       ├── request.rs
    │       ├── response.rs
    │       ├── callbacks.rs
    │       ├── handles.rs
    │       └── ffi/
    │           ├── mod.rs
    │           ├── server.rs
    │           └── request.rs
    │
    └── ...
```

Đây chỉ là ví dụ.

Agent được phép thiết kế structure tốt hơn sau khi đọc code thực tế.

---

# 8. VPM Package Organization

Vutnet hiện phát hành đúng một VPM package. VPM path dependencies hữu ích cho
phát triển cục bộ nhưng không tạo workspace namespace, và publisher từ chối
local VPM dependencies cùng nested `vpm.toml` manifests. Vì vậy HTTP engine
được giữ thành crate Cargo nội bộ thay vì child VPM package:

```text
vutnet/
├── vpm.toml
├── src/
├── native/
│   ├── Cargo.toml
│   ├── src/                Vutnet native adapter
│   └── server/             internal vutnet-server-native Cargo crate
└── specs/
```

Chỉ tạo thêm VPM package khi nó có release boundary độc lập và publisher có
thể đóng gói dependency graph đó một cách tái lập. Không dùng `vpms/` như thể
đó là VPM workspace hay package discovery mechanism.

---

# 9. Vut Is the Public Language Surface

Không để Rust implementation leak vào user-facing Vut API.

User không nên cần biết:

```text
Tokio
Hyper
Rust Future
Pin
Arc
Mutex
Rust lifetime
raw pointer
FFI ABI
```

Ví dụ user viết:

```vut
app.listen(3000)
```

không phải:

```vut
tokio.runtime().block_on(...)
```

Request data ở public Vut layer thuộc:

```vut
Context
```

không phải wrapper public tên:

```text
HyperRequest
```

Rust là implementation detail.

---

# 10. Do Not Force 1:1 Vut/Rust Structure

VPM packages và Rust crates không bắt buộc map 1:1.

Cấu trúc Vutnet đã chọn là:

```text
VPM package: vutnet
Cargo crates: vutnet-native, vutnet-server-native
External crates: tokio, hyper, http, ...
```

Một VPM archive có thể chứa nhiều Cargo crate nội bộ khi chúng được build và
phát hành cùng nhau. Chọn structure dựa trên:

- ownership;
- reuse;
- build performance;
- dependency boundaries;
- maintainability.

Không dựa vào symmetry hình thức.

---

# 11. Reuse Vut Infrastructure

Trước khi tự tạo mechanism mới, kiểm tra Vut đã có gì.

Đặc biệt:

```text
Receiver Functions
callbacks
closures
Vutcon
FFI
native packages
VPM local dependency resolution
managed values
str
bytes
list
map
result
optional
```

Nếu mechanism hiện tại giải quyết được vấn đề thì sử dụng nó.

Ví dụ Router DSL phải ưu tiên Receiver Functions thay vì tạo parser DSL riêng.

---

# 12. Do Not Modify Vut Core Without Need

Mặc định không sửa:

```text
compiler
runtime
stdlib
VPM
LSP
```

chỉ để làm Vutnet dễ implement hơn.

Trước tiên hãy tìm cách implement đúng bằng capabilities hiện tại.

Tuy nhiên:

**Không được workaround một bug/limitation thực sự bằng một public API tệ chỉ để tránh sửa core.**

Nếu Vutnet phát hiện bug tổng quát của Vut:

1. tạo minimal reproduction;
2. xác định root cause;
3. xác định component bị ảnh hưởng;
4. đề xuất general fix;
5. báo rõ tại sao Vutnet cần nó;
6. dừng phần phụ thuộc vào blocker;
7. chờ approval trước khi sửa core/compiler/runtime/VPM.

Không silently patch core.

---

# 13. Existing Code Beats Assumptions

Không assume API tồn tại chỉ vì spec nói vậy.

Ví dụ trước khi sử dụng:

```vut
callback.call(...)
```

hãy xác minh callback implementation hiện tại.

Trước khi sử dụng native package feature:

hãy đọc VPM/native implementation hiện tại.

Trước khi thiết kế FFI callback:

hãy đọc Vut runtime ABI.

Trước khi assume một type là thread-safe:

hãy chứng minh từ implementation.

**Code first, assumptions last.**

---

# 14. Receiver Functions

Routing DSL của Vutnet dựa trên Receiver Functions.

Target:

```vut
fn RootRouter() -> Router:
    router():
        Routes(path = "/admin"):
            Route(path = "/setting", controller = SettingController)
```

Không thay mechanism này bằng macro/parser/custom syntax nếu Receiver Functions hiện tại có thể đáp ứng.

Phân biệt:

```text
RouterScope
```

là builder context,

còn:

```text
Router
```

là finished configuration.

Rust native engine không cần hiểu Receiver Function DSL.

Vut side phải resolve DSL thành routing representation trước khi chuyển sang native layer.

---

# 15. Native Boundary Must Be Explicit

FFI/native boundary là phần critical nhất của project.

Mọi value crossing boundary phải có contract rõ:

```text
ownership
lifetime
encoding
allocation
deallocation
thread ownership
error semantics
```

Đặc biệt:

```text
str
bytes
Request
Response
Server
callbacks
closures
native handles
```

Không dùng unsafe Rust để che một ownership problem.

Nếu cần unsafe:

- document invariant;
- giữ unsafe surface nhỏ;
- test lifecycle;
- test invalid paths;
- giải thích trong report.

---

# 16. Callback Safety Before Framework Complexity

Trước khi build routing/middleware phức tạp, phải chứng minh:

```text
Vut callable
    ↓
registration
    ↓
native callback ID/handle
    ↓
Rust stores it
    ↓
HTTP request
    ↓
Rust invokes Vut callable
```

và:

- repeated calls an toàn;
- capturing closures nếu support phải đúng;
- không leak;
- không use-after-free;
- không call từ thread không hợp lệ;
- request/response lifecycle đúng.

Nếu callback bridge chưa ổn:

**không tiếp tục xây abstraction bên trên như thể nó đã ổn.**

---

# 17. Native-First

Vutnet V1 là native-first.

Ưu tiên:

```text
Windows
Linux
macOS
```

theo target support thực tế của Vut.

Không làm architecture native tệ đi chỉ để hỗ trợ browser.

Browser/WASM networking có model khác và có thể được thiết kế riêng sau.

WASI support cũng không phải blocker cho native MVP.

---

# 18. Testing Rules

Không coi compile success là feature success.

Cần test ở nhiều layer:

```text
Rust unit
Rust integration
FFI
Vut package
Router DSL
HTTP
End-to-end
Lifecycle
Concurrency
Stress
```

HTTP milestone chỉ COMPLETE khi flow thật hoạt động:

```text
real HTTP client
       ↓
real socket
       ↓
Rust HTTP engine
       ↓
Vutnet router
       ↓
real Vut controller
       ↓
real HTTP response
       ↓
client
```

Không mock output để claim completion.

---

# 19. Keep Existing Gates Green

Sau thay đổi phù hợp, chạy các gate liên quan:

```text
cargo fmt
cargo clippy
cargo test

vpm fmt --check
vpm lint
vpm check
```

Nếu thay đổi component của root Vut project, chạy regression tests của component đó.

Không chỉ test Vutnet rồi bỏ qua regression của compiler/runtime/VPM đã chạm vào.

---

# 20. Performance Philosophy

Vutnet hướng tới framework có performance tốt.

Nhưng:

**correctness trước micro-optimization.**

Không thêm unsafe hoặc complexity chỉ dựa trên assumption rằng nó nhanh hơn.

Đo trước.

Các metric đáng quan tâm:

```text
requests/sec
latency
allocations/request
memory growth
concurrent connections
FFI crossings/request
startup time
```

Routing configuration phải được build/flatten/register ở startup thay vì chạy Receiver DSL lại trên từng request.

---

# 21. No Premature Feature Explosion

MVP tập trung:

```text
HTTP server
App
Router
Routes
Route
Controller
Context
Response
route params
query
headers
body
```

Middleware is part of the current MVP. Global middleware uses `app.use`; scoped
middleware uses receiver `Use(middleware)`, and `UseRouter(path, router)` mounts
a RouterFactory. Request data is exposed through Context rather than a separate
controller Request argument.

Static files and multipart are implemented 0.1.x features; follow
`specs/09-static-and-multipart.md`. Use `app.static_files(prefix, directory)`
because `static` is a reserved Vut keyword. Multipart route requests preserve
typed middleware; native static responses bypass it. Do not add ambient
request-path filesystem opens or write uploads using untrusted filenames.

Typed View trees, slot/layout resolution, server-side HTML rendering, browser
mounting and SSR hydration with typed form events are implemented Vutnet features, not a template
engine; follow `specs/10-view-system.md`. Event handlers live separately from
attributes and are mapped through the existing WASM callback host into owned Vut
event snapshots. Real Chromium E2E covers form events, prevention/propagation,
refresh and listener cleanup. DOM control methods are valid only during the
synchronous callback. SSR uses a callback-free View projection. The older
`InteractiveView` click dispatcher remains an in-process compatibility API.
Captured cell-backed renderer/event callbacks are verified; arbitrary managed
capture shapes and async entry are not guaranteed. Keep
the state API and any remaining WASM ABI limitations scoped to their specs.
`../vutnet-examples/view-ssr/` and its real VPM/Hyper HTTP integration
exercise the SSR surface; `crates/vpm/tests/vutnet_view_hydration_e2e.rs` covers browser DOM
hydration, mismatch recovery and event/state cleanup. The dev pipeline test
`crates/vpm/tests/vutnet_dev_hydration_e2e.rs` also exercises automatic client
build, asset serving, bootstrap and click-driven state/DOM updates over a real
Vutnet server in dev, run and production start. Incremental DOM commits and
unique-ID list moves and typed fine-grained bindings are implemented. Follow
`specs/14-fine-grained-view.md`: `text(fn() -> str)`,
`bind_attrs(fn() -> list[Attribute])`, and exclusive `children(fn() -> View)`.
Keep dependency collection isolated and cleanup scoped to each binding/region.
Eager reads outside callbacks retain full-render compatibility; do not claim
that evaluated strings automatically become reactive bindings.

Không ưu tiên sớm:

```text
ORM
template engine
GraphQL
WebSocket
TLS abstraction
sessions
cookies
JSON framework
compression
plugin ecosystem
```

Các feature này có thể thêm sau khi HTTP/controller foundation ổn định.

---

# 22. Documentation Must Follow Reality

README/spec/examples phải phản ánh implementation thực tế.

Không document API chưa tồn tại như thể đã hoàn thành.

Experimental API phải được đánh dấu rõ.

Nếu implementation thay đổi architecture hợp lý:

hãy cập nhật specs liên quan.

---

# 23. Agent Workflow

Khi bắt đầu một phase:

### Step 1 — Read

Đọc:

- relevant Vutnet specs;
- relevant root Vut specs;
- implementation hiện tại;
- tests hiện tại.

### Step 2 — Verify assumptions

Xác minh:

- API tồn tại;
- syntax hợp lệ;
- runtime behavior;
- ownership;
- build pipeline.

### Step 3 — Plan

Lên implementation plan nhỏ cho phase.

### Step 4 — Implement

Được phép:

- tạo file;
- tạo folder;
- chia module;
- thêm crate;
- thêm VPM dependency;
- refactor local Vutnet code.

### Step 5 — Test

Chạy relevant gates.

### Step 6 — Report

Báo:

```text
Status
Implemented
Architecture
Dependencies added
Files created
Files changed
Tests
Native changes
Core changes
Known limitations
Blockers
Next phase
```

---

# 24. Reporting Must Be Honest

Status chỉ dùng:

```text
COMPLETE
PARTIAL
BLOCKED
NOT STARTED
```

Không ghi COMPLETE nếu Definition of Done chưa đạt.

Nếu 90% xong nhưng critical path chưa hoạt động:

```text
PARTIAL
```

Nếu compiler/runtime blocker:

```text
BLOCKED
```

Không che blocker bằng workaround không đúng.

---

# 25. Final Engineering Principle

Vutnet không cần tự xây mọi thứ.

Vutnet cần xây **đúng abstraction cho Vut** trên nền những system libraries tốt nhất có sẵn.

Ưu tiên:

```text
reuse proven Rust crates
        +
small explicit native boundary
        +
typed Vut API
        +
Receiver Function DSL
        +
modular architecture
        +
real end-to-end tests
```

Thay vì:

```text
reimplement everything
        +
giant modules
        +
unsafe shortcuts
        +
spec-driven assumptions
        +
mocked success
```

Mục tiêu cuối cùng không phải chứng minh rằng Vut có thể tự viết HTTP stack.

Mục tiêu là tạo ra một web framework mà developer Vut có thể sử dụng đơn giản:

```vut
import vutnet
import vutnet.router at router, Router

fn RootRouter() -> Router:
    router():
        Route(path = "/", controller = HomeController)

fn main() -> int:
    app = vutnet.app()
    app.router(RootRouter)
    app.listen(3000)
    0
```

trong khi Rust đảm nhiệm system-level complexity phía dưới một cách nhanh, an toàn và chuyên nghiệp.



# 28. Native package naming

Vut already has an `http` standard-library module for outgoing requests. Vutnet
ships one VPM package and keeps its HTTP engine in the internal Cargo crate
`vutnet-server-native` under `native/server`; it is not a VPM child package or
public Vut namespace. Do not create a VPM package named `http`.
