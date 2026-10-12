# Vutnet — Overview

## 1. Mục tiêu

Vutnet là web framework native cho Vut. Vut cung cấp API ứng dụng, Router DSL,
Context, Controller và Response có kiểu tĩnh. Rust cùng các crate networking
đảm nhiệm HTTP server và I/O.

API đích của MVP:

~~~vut
import vutnet
import router

fn main() -> int:
  app = vutnet.app()
  app.router(router.RootRouter)
  app.listen(3000)
  0
~~~

Controller luôn có cùng signature:

~~~vut
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

type Controller = fn(Context) -> Response

fn HomeController(ctx: Context) -> Response:
  return response.send(200, "Hello Vutnet")
~~~

Router phải yêu cầu Controller đúng signature ngay khi biên dịch. Không nhận
callable tùy ý, không hạ thành dyn và không dùng runtime reflection để kiểm tra.

## 2. Request và response flow

Đây là model duy nhất của Vutnet:

~~~text
native HTTP request
        ↓
parse và validate request
        ↓
match route, tạo Context có lifetime giới hạn trong lần gọi controller
        ↓
global middleware → Router/Routes middleware → Controller(Context)
        ↓ Response unwind theo onion order
convert Response sang native HTTP response
        ↓
client
~~~

Context chứa thông tin request cần cho ứng dụng thông thường. Controller
không nhận một Request riêng hay mutable response parameter. Response là giá
trị được tạo từ module vutnet.response và trả về.

## 3. Phân chia trách nhiệm

Vutnet không implement HTTP server trực tiếp. VPM phát hành một package duy
nhất tên `vutnet`; bên trong native Cargo crate `vutnet-server-native` sở hữu
Hyper/Tokio transport và được build/link cùng native adapter.

~~~text
vutnet (một VPM package)
├── Vut API: App, Router DSL, Context, Response, View
├── native adapter: route matching, callback, Context/Response mapping
└── native/server: internal Cargo crate
    ├── Tokio listener/runtime
    ├── Hyper HTTP server
    ├── bounded request body
    ├── request/response transport
    └── listener/connection lifecycle
        ↓
Hyper + Tokio
        ↓
OS/network
~~~

Vut source gọi RouterFactory và `Router.flatten()` một lần; typed
`list[RouteEntry]` đi qua public ABI tới adapter. Adapter giữ route metadata và
callback handles rồi dispatch request tới internal server engine. Rust crate
`vutnet-server-native` không phải VPM dependency hay public Vut namespace; nó
không sở hữu Router, Controller hoặc application semantics. Tokio/Hyper cũng
đáp ứng network/HTTP infrastructure nên không cần package `net` riêng.

Rust không parse hay thực thi Receiver DSL. Vut source gọi RouterFactory và
`Router.flatten()` một lần; native adapter chỉ nhận route list đã flatten, rồi
giữ metadata và callback handles cho các request tiếp theo.

## 4. Vut capabilities và giới hạn đã xác minh

- Function values và function types dạng fn(Args) -> Result hiện có.
- Type alias có trong grammar/type system; parser nhận alias function type như
  type Handler = fn(int) -> int. Alias không tạo nominal type mới.
- Receiver function type dùng cú pháp riêng fn(Receiver)(Args) -> Result.
- Receiver methods đã visible xuyên module và xuyên VPM local package, kể cả
  receiver method có ordinary argument đi kèm trailing receiver callback (ví
  dụ `Column(name, body)`). Điều này đã được fix và regression-test bởi
  `crates/vpm/tests/receiver_package_e2e.rs` và
  `crates/vut-compiler/tests/receiver_module_e2e.rs`. Vì vậy `app.router`,
  `app.listen`, các `ctx.*` accessors và Router DSL dùng được từ module ứng
  dụng; đây không còn là blocker.
- Router DSL (Phase 3.2) đã được implement bằng pure Vut: `Router`,
  `RouterScope`, `RoutesScope`, `RouteEntry`, `RouterError`, `router()`,
  `Route`/`Routes`/scope `Use`/`UseRouter` và `Router.flatten()`, với path normalization
  deterministic và duplicate rejection. Xem 02-router-dsl.md.
- Generic bounds trên interface đã được dùng trong stdlib và compiler tests.
- Optional viết T?; Result viết result[T, E]; str và bytes là hai type riêng.
- stdlib json hiện có json.Value, json.Encodable, json.encode và json.stringify.
- Direct FFI callback C chỉ nhận non-capturing function/lambda với tham số và
  kết quả được ABI công khai hỗ trợ.

Root FFI ABI v1 không cho truyền data, interface, optional, result, str, bytes
hoặc enum by-value qua C ABI. Vì vậy source-level Controller, Context và
Response chưa chứng minh được trực tiếp làm callback C bằng direct C function
pointer. Managed Vut callable dùng public Native Callback ABI riêng, hiện đang
được proof ở Phase 1; không được giả định alias type làm thay đổi ABI và không
được làm yếu Controller thành dyn hoặc raw pointer trong API.

Native Callback ABI v1 có typed list operations, nominal data build/access và
callable-field promotion. Compiler chọn typed callback-ABI import path khi kiểu
tham số của native import có callable trong type graph. `RouteEntry.controller`
đáp ứng điều kiện này, nên source vẫn khai báo `list[RouteEntry]` có kiểu và
Rust nhận được route list qua public value handles. ABI/compiler tests và real
HTTP E2E của Phase 3 đã qua. ABI v1 hiện chỉ promote named/non-capturing
callables; callback dispatch serialized, và Vut trap vẫn đi vào abort path.

Các bằng chứng chính nằm trong root specs/02-type-system.md,
specs/04-functions-methods.md §§45–46a, specs/receiver/,
specs/ffi/06-native-abi-v1.md, specs/ffi/07-native-callback-abi-proposal.md,
crates/vut-parser/src/parser.rs, crates/vut-types/src/checker/analyze.rs và
crates/vut-compiler/tests/ffi_e2e.rs.

## 5. Phạm vi MVP

MVP tập trung:

- HTTP server foundation do internal `vutnet-server-native` Cargo crate đảm nhiệm;
- App, Router, Route, Routes group, scoped middleware `Use` và `UseRouter` để
  mount RouterFactory;
- Controller(Context) -> Response;
- Context accessors cho method, path, params, query, headers, body và metadata;
- response.send và response.send_bytes cho text/binary;
- response.json cho json.Value và response.json_encodable cho các model opt-in
  qua json.Encodable;
- response headers bất biến có validation và replacement không phân biệt hoa
  thường;
- typed middleware `fn(Context, Next) -> Response`, đăng ký toàn app bằng
  `app.use(...)`, đăng ký theo Router/Routes scope bằng `Use(...)`, với
  inheritance xác định, onion unwind và short-circuit;
- App.with_max_body_bytes cho giới hạn request body;
- App.static_files cho static mounts an toàn, MIME và native file streaming;
- Context.multipart cho typed fields/files, repeated values và binary uploads;
- `vutnet.view` cho typed View trees, deterministic layout/slot resolution,
  HTML SSR và overloaded typed `response.render(Page)` / `response.render(Page, data)`
  qua public managed-value ABI; browser `mountView` dispatch typed DOM events
  và refresh View khi state cell được ghi; opt-in `render_hydratable` plus
  `hydrateView` validates the shared plan and reuses initial SSR DOM;
- App.listen dừng nhận request và drain kết nối khi có process interrupt;
- GET mặc định khi endpoint bỏ method;
- lỗi 404/405/413, response status/header không hợp lệ và request lỗi được xử
  lý xác định.

Không đưa vào MVP: template engine, `vut.render`,
dedicated keyed-list API, public streaming Response API, WebSocket hay TLS
abstraction. SSR tree/layout API của `vutnet.view` có real-socket E2E; typed
event dispatch, typed fine-grained text/attrs/children bindings, eager compatibility,
initial SSR hydration và listener/subscription cleanup có Chromium E2E. Binding
chỉ tính lại callback phụ thuộc cell đã ghi; eager reads giữ full-render
compatibility. Xem `14-fine-grained-view.md`; HTML id giữ identity cho list/component khi reorder. Static/multipart 0.1.x nằm trong
`09-static-and-multipart.md`.

## 6. Tên và package

stdlib đã có module import tên `http` dành cho outgoing client. Vutnet không
tạo VPM child package tên `http` hay `server`: package registry phát hành một
archive duy nhất, còn `vutnet-server-native` là crate Cargo nội bộ tại
`native/server`. Crate này sở hữu listener, connection lifecycle, bounded
request body, request/response transport và server errors; nó không chứa Router
DSL, Controller, Context, middleware hay application behavior. Tokio/Hyper
cung cấp network/HTTP infrastructure nên cũng không cần package `net` riêng.

`Routes(path = "/"):` receiver group đã compile và chạy được với compiler hiện
tại; nested receiver DSL và trailing receiver callback đã được chứng minh qua
regression tests (`receiver_module_e2e`, `receiver_package_e2e`). V1 dùng
`Use(middleware)` cho middleware scope và `UseRouter(path, router)` để mount
RouterFactory; middleware được giải quyết sau khi Router body chạy xong nên
registration viết sau Route vẫn áp dụng. Đây là behavior đã được kiểm tra bằng
socket E2E.

Chi tiết package, layout và dependency direction ở 06-packages-and-modules.md.
Chi tiết native server engine ở 08-server-package.md.

## 7. Quy tắc source of truth

Các tài liệu này là design reference. Khi khác root language/FFI specs hoặc
implementation, ưu tiên root specs và implementation. Thay đổi compiler,
runtime, stdlib hay VPM cần reproduction và approval riêng; task cập nhật specs
không thực hiện các thay đổi đó.

## 8. Official CLI

The official `vutnet` command is a Vut `[[bin]]` entrypoint. It provides
`new`, `dev`, `build`, `start`, `routes`, and `info`; typed native imports route
low-level work to Rust. The persistent Vutnet-aware development supervisor
belongs to this CLI; `vpm watch` remains the generic, manifest-configured
process watcher. For each `response.render_hydratable(Page[, data])` call,
VPM generates a typed browser Page registry and Vutnet automatically builds and
serves the WASM client and bindings. This bootstraps the existing `hydrateView`
against hydratable SSR pages without a user client bin or `[browser]` config.
This does not change `response.render`. `run`/`build` package browser assets;
`start` uses the validated release bundle without compiling. See `11-cli.md`
and `13-ssr-hydration.md` for registry generation and lifecycle.
