# Rcli

一个用 Rust 写的命令行工具箱，提供 **5 组能力**：

| 命令 | 作用 | 典型场景 |
|---|---|---|
| `rcli csv` | CSV → JSON / YAML / TOML | 把表格数据喂给别的工具 |
| `rcli genpass` | 生成随机密码 + 强度评估 | 脚本里 `pass=$(rcli genpass)` |
| `rcli base64` | 文件/标准输入的 Base64 编解码 | 传二进制、拼 URL、调试 JWT |
| `rcli text` | 文本签名与验签（blake3 对称 / ed25519 非对称） | 给发布物签名、校验文件未被篡改 |
| `rcli http` | 把本地目录起成一个静态 HTTP 服务 | 局域网分享文件、前端联调 |

同时它也是一个**"Rust CLI 工程化样板"**：`cli`/`process` 分层、trait 分发、bin+lib 双 target、
pre-commit 六道关卡、cargo-deny / typos / git-cliff / GitHub Actions 一应俱全。

> 本文的写法约定：讲"这个项目做什么、怎么跑、怎么测"之外，**每一处结论都标了是否实测**。
> 未标注"实测"的是设计意图推断；标了"实测"的都在当前 HEAD 上用 release 版二进制真跑过。
> 环境：Windows 11 + Git Bash，`rustc 1.99.1 (LLVM 23.1.1)`，edition 2021。

---

## 1. 30 秒看懂运行逻辑

```
argv (字符串)
   │
   ▼  ① 协议层：clap derive 解析 + value_parser 校验        src/cli/*
 Opts { cmd: SubCommand }        ← 到这一步，字符串已经变成"有类型的 enum/struct"
   │
   ▼  ② 分发层：opts.cmd.execute()                          src/main.rs（只有一行）
   │     SubCommand ─┐
   │                 ├─ #[enum_dispatch(CmdExecutor)] 自动生成 match + .await
   │     Base64SubCommand / TextSubCommand / HttpSubCommand ┘（二级同理）
   │
   ▼  ③ 业务层：process_* 函数，只接 &str / PathBuf / enum   src/process/*
 get_reader → stdin 或文件 ；serde → 三种格式；ed25519/blake3 → 签名；axum → HTTP
   │
   ▼  ④ 输出
 stdout（结果）/ stderr（提示与错误）/ 落盘文件 / HTTP 响应
```

一句话：**`cli/` 把字符串升级成类型，trait 把类型分发到实现，`process/` 只干活，`main.rs` 只有三行。**

三段各自的可替换性：

| 层 | 认识什么 | 不认识什么 | 换掉的代价 |
|---|---|---|---|
| `cli/`（协议） | clap、命令行拼写、参数校验 | 业务规则 | 换 argh/clap 大版本只动这一层 |
| trait 分发 | `CmdExecutor`、enum variant | 任何具体业务 | 手写 match ↔ enum_dispatch 互换 |
| `process/`（业务） | `&str`/`PathBuf`/领域 enum | `CsvOpts`、`clap` | 可原样搬进 Web 服务/库给别的 crate 用 |

---

## 2. 目录结构，以及每个文件为什么存在

```
src/
├── main.rs                 # bin target：init 日志 → Opts::parse() → cmd.execute()
├── lib.rs                  # lib target：crate 门面。⚠️ trait 必须声明在 mod 之前（见 §3.2）
├── utils.rs                # get_reader()：把 "-" 或路径统一成 Box<dyn Read>
├── cli/                    # ① 协议层
│   ├── mod.rs              #   Opts / SubCommand / verify_file / verify_path
│   ├── csv.rs              #   CsvOpts + OutputFormat（From/FromStr/Display 三向转换）
│   ├── genpass.rs          #   GenPassOpts（全是默认值声明）
│   ├── base64.rs           #   Base64SubCommand + Encode/DecodeOpts + Base64Format
│   ├── text.rs             #   TextSubCommand + Sign/Verify/KeyGenerateOpts + TextSignFormat
│   └── http.rs             #   HttpSubCommand + HttpServeOpts
└── process/                # ③ 业务层（一个能力一个文件）
    ├── mod.rs              #   mod 声明 + pub use process_*（新增功能在这里加两行）
    ├── csv_convert.rs      #   读 CSV → Vec<serde_json::Value> → 三种文本
    ├── gen_pass.rs         #   字符池 → 保证覆盖 → 洗牌
    ├── b64.rs              #   编解码 + 2 个单元测试
    ├── text.rs             #   TextSign/TextVerify/KeyLoader/KeyGenerator 四个 trait + 2 个测试
    └── http_serve.rs       #   axum Router + file_handler + 目录索引 + 3 个 #[tokio::test]

assets/juventus.csv         # 手工试数据（球队名单，字段类型杂）
fixtures/                   # 测试夹具：b64.txt / blake3.txt / ed25519.sk / ed25519.pk / index.html
msg.txt                     # 签名演练用的消息文件（内容 "hello!"）
test.rest                   # REST Client（VSCode 插件）用例，配合 http serve
output.{json,yaml,toml}     # csv 产物，已 gitignore
Cargo.toml / Cargo.lock     # 依赖 + 锁版本
.pre-commit-config.yaml     # 提交前 12 个 hook（见 §7）
cliff.toml / CHANGELOG.md   # git-cliff：从 conventional commits 生成日志
deny.toml                   # cargo-deny：许可证 / 漏洞 / 重复依赖 / 来源
_typos.toml                 # typos：拼写检查
.github/workflows/build.yml # CI：fmt → check → clippy -D warnings → nextest → tag 发 release
```

### 2.1 为什么一个包里有 `main.rs` 和 `lib.rs` 两个 target

`Cargo.toml` 没有 `[[bin]]`，cargo 自动发现 `src/main.rs` → 可执行文件 `rcli`，`src/lib.rs` → 同名库。
好处有三个，都很实在：

1. **业务代码可被直接调用与断言**：`use rcli::process_encode` 就能测，不需要 spawn 进程再解析 stdout。
2. **`main.rs` 才能保持三行**：它是 bin crate，只能看见 lib 门面 `pub use` 出来的符号。
3. **可见性边界由编译器把关**：`mod cli;`/`mod process;` 是私有的，公开面就是 `lib.rs` 那一段 `pub use`，一眼能数清。

**两个 crate 是隔离的**（实测验证过）：`src/main.rs` 里没有任何 `mod` 声明，所有源文件只通过 `lib.rs` 的 `mod` 链编进**库 crate** 这一棵树；bin crate 是独立编译单元，只能写 `use rcli::Xxx`。
推论：lib 内部自引用必须写 `crate::`，写 `rcli::` 会因"对自己不可见"而失败。

### 2.2 为什么 `cli/mod.rs` 里到处写 `self::`

crate 同时有本地 `mod csv` 和外部 crate `csv`、本地 `mod base64` 和外部 crate `base64`。
Rust 2018+ 下裸路径 `use csv::X` 有歧义。`self::` = "当前模块下的那个"，`super::` = 上一层，`crate::` = 从 crate 根开始。

### 2.3 为什么校验函数放在 `cli/mod.rs`

`verify_file` / `verify_path` 是"参数能不能接受"的判断，属于**协议层**，
所以写成 `cli/mod.rs` 里的私有顶层函数，被 `csv.rs`、`base64.rs`、`text.rs`、`http.rs` 用 `super::` 共享——**只写一次，五个命令都受用**。

---

## 3. 分发机制：`CmdExecutor` + `enum_dispatch`

这是本项目最"设计导向"的一块，单独讲。

### 3.1 抽象：一个 async trait 当分发契约

```rust
// src/lib.rs
#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait CmdExecutor {
    async fn execute(self) -> anyhow::Result<()>;
}
```

- **消费 `self`（而不是 `&self`）**：子命令执行时要把字段逐个"交出去"给 `process_*`，移动语义最自然。
- **`async`**：只有 `http serve` 真的需要 await（axum），但 `csv`/`genpass` 这类同步实现写 `async fn` 也**零成本**——`enum_dispatch` 是静态分发，不匹配的那个分支根本不会被执行。
- **`#[allow(async_fn_in_trait)]`**：带 `async fn` 的 trait 不是 dyn-compatible（老名字"对象安全"），编译器会出 `explicit_future_to_dyn` 提示；本项目**永远只用静态分发**，所以直接放行。

叶子实现各自待在自己的文件里（**协议层只做"转调业务层"**）：

```rust
// src/cli/csv.rs
impl CmdExecutor for CsvOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let output = if let Some(o) = self.output { o } else { format!("output.{}", self.format) };
        crate::process_csv(&self.input, output, self.format)   // 注意：没有 .await
    }
}
```

> `process_csv` 是普通同步函数，在 `async fn` 里直接调用即可——**不需要**给每个 `process_*` 都加 `async`。

### 3.2 `enum_dispatch` 生成了什么（以及那个必须记住的顺序坑）

标注只有两行：

```rust
#[enum_dispatch]              // ① trait 侧（src/lib.rs）
#[enum_dispatch(CmdExecutor)]  // ② enum 侧（SubCommand / Base64SubCommand / TextSubCommand / HttpSubCommand）
```

宏为每个 enum 生成的代码大致是（实测由 `cargo check` 通过反推，形态与 [expansion.rs](file:///C:/Users/Admin/.cargo/registry/src/mirrors.ustc.edu.cn-38d0e5eb5da2abae/enum_dispatch-0.3.13/src/expansion.rs) 一致）：

```rust
impl CmdExecutor for SubCommand {
    async fn execute(self) -> anyhow::Result<()> {
        match self {
            Self::Csv(inner)      => CsvOpts::execute(inner).await,
            Self::GenPass(inner)  => GenPassOpts::execute(inner).await,
            Self::Base64(inner)   => Base64SubCommand::execute(inner).await,
            ...
        }
    }
}
// 另外白送：impl From<CsvOpts> for SubCommand、impl TryInto<CsvOpts> for SubCommand
```

三条硬约束（**都是本项目踩过的**）：

1. **trait 必须声明在 `mod cli;` 之前**。宏用"缓存 + 延迟链接"配对：trait 侧 `#[enum_dispatch]` 展开时，会把**先于它缓存到的 enum** 的 dispatch 代码生成到 **trait 所在的文件位置**。原来 `mod cli;` 写在 trait 前面，生成的代码落到 crate 根，那里看不到私有 `mod cli` 里的 `CsvOpts`/`Base64EncodeOpts`，直接报一堆 `E0425 (cannot find type ... exists but is inaccessible)` + `E0119 (conflicting TryInto)`。**顺序修正后编译零警告。**
2. **调用点必须把 trait 放进作用域**：`main.rs` 要 `use rcli::{CmdExecutor, Opts};`，否则 `opts.cmd.execute()` 报 `no method found`——**trait 方法不会因为你调的是具体类型就自动可见**。
3. **手写 `impl CmdExecutor for SubCommand` 必须删掉**，否则与宏生成的实现冲突（E0119）。

### 3.3 为什么不用 `dyn CmdExecutor`

`async fn` in trait 不是 dyn-compatible，`Box<dyn CmdExecutor>` 这条路根本走不通（除非上 `async-trait` 做装箱）。`enum_dispatch` 是**静态分发**：无堆分配、无虚表、可内联，且生成的就是普通 `match`。代价是"新增实现要登记"——而 variant 本身就是登记处，正合 CLI 的场景。

### 3.4 二级嵌套分发也走同一条路

`SubCommand::Base64(Base64SubCommand)` → 宏生成的分支调用 `Base64SubCommand::execute(inner).await` → 而 `Base64SubCommand` 自己也被 `#[enum_dispatch(CmdExecutor)]` 标注 → 再分发到 `Base64EncodeOpts`。
**实测**：`rcli base64 encode`、`rcli text sign`、`rcli http serve` 三条二级路径在 debug 与 release 下都正常。

### 3.5 一条命令的完整生命周期（以 `text sign` 为例，跨两个 crate）

```
rcli text sign -i msg.txt -k fixtures/blake3.txt
```

1. `main.rs`：`tracing_subscriber::fmt::init()`（日志，默认级别见 §5.5）
2. `main.rs`：`Opts::parse()` → clap 读 argv → `SubCommand::Text(TextSubCommand::Sign(TextSignOpts{...}))`
   - `-i` 的 `value_parser = verify_file` 在**进入结构体之前**就拦下不存在的路径；
   - `--format` 的 `value_parser = parse_format` 把 `"blake3"` 变成 `TextSignFormat::Blake3`。
3. `main.rs`：`opts.cmd.execute().await?` → 宏生成的 match → `TextSubCommand` → `TextSignOpts::execute`
4. `cli/text.rs`：把 `self.input/self.key/self.format` 交给 `crate::process_text_sign(...)`，拿到 `String` 后 `println!`
5. `process/text.rs`：`get_reader(input)?`（`Box<dyn Read>`）→ `Blake3::load(key)?`（`KeyLoader`）→ `signer.sign(&mut reader)?`（`TextSign`）→ `URL_SAFE_NO_PAD.encode(...)`
6. 任一步 `?` 失败 → 冒泡到 `main` 的 `-> anyhow::Result<()>` → Rust 的 `Termination` 打印 `Error: xxx` 到 stderr，**退出码 1**（实测）。

---

## 4. 怎么运行

### 4.1 准备

```bash
rustup component add rustfmt clippy        # 关卡需要
cargo install cargo-nextest cargo-deny typos git-cliff --locked   # 可选，跑全套关卡需要
```

实测本机版本：`cargo-nextest 0.9.149`、`cargo-deny 0.20.2`、`typos-cli 1.51.1`、`git-cliff 2.14.2`；
`pre-commit` **未安装**（`.pre-commit-config.yaml` 里的 hook 因此在本机没跑过，CI 才跑得到）。

### 4.2 日常开发

```bash
cargo run -- <子命令> [参数]        # debug 构建（有溢出检查、有 debug_assert）
cargo run -q -- <子命令> [参数]     # -q 关掉 cargo 自身的输出，只看程序 stdout
cargo build                         # 只编译，产物 target/debug/rcli.exe
cargo check                         # 只做类型检查，最快
```

### 4.3 装到 PATH 上

```bash
cargo build --release               # 先编译，再 install 可复用产物（实测 6.45s）
cargo install --path . --locked     # ⚠️ 建议一定带 --locked
~/.cargo/bin/rcli.exe --version     # → rcli 0.1.0（实测）
cargo uninstall rcli                # 卸载
```

**为什么必须 `--locked`**（实测）：`cargo install --path .` 不带 `--locked` 时**不会用仓库的 `Cargo.lock`**，而是重新解析一遍依赖树，日志里会出现
`Locking 191 packages` / `Adding matchit v0.8.4 (available: v0.8.6)`，装出来的东西和 CI、本地 debug 构建的依赖版本不一致。

**一个真实的偶发故障**（实测复现过）：不带 `--locked` 那次，rustc 在编译第三方 `regex v1.13.1` 时崩溃：

```
error: could not compile `regex` (lib)
Caused by:
  process didn't exit successfully: `...rustc.exe ... -C opt-level=3 ...`
  (exit code: 0xc0000005, STATUS_ACCESS_VIOLATION)
```

判定要点：**没有任何 `error[E0xxx]` 诊断**，且崩的不是本项目代码 → 是编译器进程访问违例（当时 32 GB 内存只剩 2.2 GB 空闲；`cargo install` 默认按 CPU 核数并发跑多个 `-C opt-level=3` 的 rustc）。
处理：重试 / `-j 1` 降并发 / 先 `cargo build --release` 再 install。注意 `0xc0000005`（崩溃）与 `0xc000013a`（STATUS_CONTROL_C_EXIT，Ctrl+C 的正常退出码）**是两回事**。

### 4.4 debug 与 release 的行为差异（重要）

`cargo build`(debug) 默认 `overflow-checks = true`，release 默认 `false`。这个项目**曾经**正好踩在边界上：
`genpass` 的长度校验缺失时，`length - 已保证位数` 是 u8 减法——debug 版 `panicked: attempt to subtract with overflow`（退出码 101），
release 版**不报错**，`-l 3` 四开关全开会静默回绕成一个 259 字符的密码（两处都是当时的实测记录）。

现在 `process_genpass` 开头用 `anyhow::ensure!` 把长度问题拦成了统一的 Err，两种构建行为一致（实测）：

```bash
./target/release/rcli.exe genpass -l 3 --upper --lower --number --symbol
# Error: password length 3 is too short: 4 character classes are enabled and each needs one slot, so the minimum length is 4
# 退出码 1；debug 版完全相同（回归测试 test_genpass_too_short_is_error_not_panic 守着它）
```

留下的教训仍然成立：**同一段代码在 debug 和 release 下可能给出完全不同的错误形态**，
所以跑测试/手工验收统一用 debug（`cargo test` 默认就是 debug），只在最后验收产物时用 release。

---

## 5. 怎么使用（全部实测）

### 5.1 通用约定

- **`-i -`（或直接不给 `-i`）表示读 stdin**：`verify_file` 对 `"-"` 放行，`get_reader` 把 `"-"` 翻译成 `std::io::stdin()`。
- **stdout 只放结果，stderr 放提示**：`genpass` 的密码走 stdout、`Password strength:` 走 stderr，因此 `pass=$(rcli genpass)` 拿到的是干净密码（实测）。
- **退出码**（实测）：

  | 码 | 含义 | 触发例子 |
  |---|---|---|
  | 0 | 成功 | 正常输出 |
  | 1 | 业务错误（anyhow 冒泡，打印 `Error: ...`） | `base64 decode` 格式不匹配 → `Error: Invalid padding`；`genpass -l 3` → `Error: password length 3 is too short: ...`；blake3 密钥不足 32 字节 → `Error: blake3 key must be exactly 32 bytes, got 5` |
  | 2 | clap 用法/参数校验错误 | 缺 `--sig`；`csv -i not-exist.csv` |
  | 101 | panic（Rust 运行时） | **当前没有已知触发路径**。原先的两处（`genpass -l 3` 下溢、blake3 短密钥越界）已改成 Err（见 §8）；一旦看到 101，说明撞上了新 bug |

- **终止进程的退出码不要按 Unix 习惯猜**（本项目环境实测）：用 `kill` 终止 `rcli http serve` 得到 **143**（128 + SIGTERM）；而在交互终端按 Ctrl+C 停原生 Windows exe，shell 报的是 `STATUS_CONTROL_C_EXIT (0xc000013a)` 而不是 130。脚本里请只判断"0 / 非 0"，不要写死数字。

### 5.2 `rcli csv` — 表格转格式

```bash
rcli csv -i assets/juventus.csv                      # 默认 -f json，写出 output.json
rcli csv -i assets/juventus.csv -f yaml -o out.yaml
rcli csv -i assets/juventus.csv -f toml
```

| 参数 | 说明 | 现状 |
|---|---|---|
| `-i/--input` | 必填，`value_parser=verify_file`（不存在直接拒绝） | 有效 |
| `-o/--output` | 不给则 `output.<format>` | 有效 |
| `-f/--format` | `json`(默认) / `yaml` / `toml` | 有效 |
| `-d/--delimiter` | 分隔符，默认 `,` | ⚠️ **声明了但没接线**（见 §8） |
| `--header` | 首行是否当表头 | ⚠️ **同上** |

三种格式实测差异（同一份数据）：

```jsonc
// output.json —— 每行一个对象，所有值都是字符串（没有类型推断）
[ { "DOB": "Apr 18, 1990 (29)", "Kit Number": "1", "Name": "Wojciech Szczesny", ... } ]
```

```yaml
# output.yaml —— 顶层就是序列
- DOB: Apr 18, 1990 (29)
  Kit Number: '1'        # 注意被加了引号：yaml 不引号会把 1 读成数字
```

```toml
# output.toml —— TOML 根必须是表，所以数组被挂在 rows 下
[[rows]]
DOB = "Apr 18, 1990 (29)"
"Kit Number" = "1"       # 含空格的键必须加引号
```

心智：**同一份 `Vec<serde_json::Value>` 交给不同后端就得到不同文本**——serde 定义的是"数据模型"，不是"JSON 解析器"。
TOML 那段是硬约束：直接序列化 `Vec<Value>` 会报 `unsupported array type`，所以代码里用 `json!({ "rows": ret })` 包一层。

### 5.3 `rcli genpass` — 随机密码

```bash
rcli genpass                          # 默认 length=16, number+symbol 开，upper/lower 关
rcli genpass -l 20 --upper --lower --number --symbol
pass=$(rcli genpass -l 16 2>/dev/null)   # 强度提示在 stderr，命令替换只拿到密码（实测）
```

算法（`process/gen_pass.rs`）：① 先检查 `length` 容不容得下“每个启用类各占一位”的保证位（不够就 `ensure!` 报 Err）→ ② 每个启用类**先各取一个** → ③ 剩余位数从合并池随机 → ④ **整体 `shuffle`**（否则前四位固定"大小写数字符号"，位置规律泄露）。
字符池用 `&[u8]` 字节常量（`const` 必须写类型），全程 `Vec<u8>`，最后一步才 `String::from_utf8`。

```bash
rcli genpass -l 3 --upper --lower --number --symbol
# Error: password length 3 is too short: 4 character classes are enabled and each needs one slot, so the minimum length is 4   （实测，退出码 1）
```

### 5.4 `rcli base64` — 编解码

```bash
rcli base64 encode -i Cargo.toml                          # 文件输入
printf 'Hello, rcli base64!' | rcli base64 encode          # stdin（不给 -i 时默认 "-"）
rcli base64 encode -i Cargo.toml -f url-safe               # 两种字母表
rcli base64 decode -i fixtures/b64.txt -f url-safe         # → Hello, rcli base64!
```

实测：`Hello, rcli base64!`

| 格式 | 输出 | 用途 |
|---|---|---|
| `standard`(默认) | `SGVsbG8sIHJjbGkgYmFzZTY0IQ==` | 邮件/MIME |
| `url-safe` | `SGVsbG8sIHJjbGkgYmFzZTY0IQ` | URL/JWT（`+` `/` `=` 会被 URL 转义吃掉） |

- `url-safe` 这个名字是 `ValueEnum` 把 `UrlSafe` 自动转 kebab-case 得来的，不是手写字符串。
- 格式和输入**必须配对**：`url-safe` 编码的内容用默认 `standard` 解码 → 实测报 `Error: Invalid padding`（退出码 1）。
- `decode` 内部会 `trim()`，因为管道/Windows 行尾会混进 `\n`、`\r\n`，而引擎不接受填充位之后的空白。
- **decode 的输出是原文**（已修，实测）：

  ```
  $ printf 'SGVsbG8sIHJjbGkgYmFzZTY0IQ' | rcli base64 decode -f url-safe
  Hello, rcli base64!
  ```

  以前这里打的是 `println!("{:?}", ret)`，即字节数组的 debug 形式 `[72, 101, 108, ...]`（早期重构到 `CmdExecutor` 时丢了
  `String::from_utf8` 那一步）。现在 `cli/base64.rs` 显式做 `String::from_utf8(ret)?` 再输出——**把字节变文本的决策放回调用方**。

- **二进制往返**是合法的：把 PNG 编码再解码回来，`decode()` 不报错；只有 `from_utf8` 会报，因为**"base64 解得动" ≠ "内容是人类文本"**。

### 5.5 `rcli text` — 签名与验签

三种子命令，两种算法：

| `--format` | 算法 | 密钥 | 能做什么 |
|---|---|---|---|
| `blake3`(默认) |  keyed hash（对称） | 一个 32 字节共享密钥 | 签名 + 验签**用同一把钥匙**，任何拿到密钥的人都能伪造 |
| `ed25519` | 非对称签名 | `ed25519.sk`(私钥 32B) / `ed25519.pk`(公钥 32B) | 私钥签、公钥验；公钥可公开分发 |

⚠️ **`text sign` / `text verify` 的 `--format` 没有 `-f` 短参数**（`text generate` 有）。实测 `text sign ... -f ed25519` 会报 `error: unexpected argument '-f'`，必须写 `--format ed25519`。

**完整闭环（实测，可直接复制）**：

```bash
# ① 生成密钥到指定目录（-o 必须是已存在的目录，由 verify_path 校验）
mkdir -p keys
rcli text generate -f blake3  -o keys          # → keys/blake3.txt （32 字节）
rcli text generate --format ed25519 -o keys    # → keys/ed25519.sk + keys/ed25519.pk（各 32 字节）

# ② 签名
SIG=$(rcli text sign -i msg.txt -k keys/blake3.txt)
echo "$SIG"        # msg.txt 内容固定时，用仓库自带夹具实测得 63P40ONwLHPxqPMTQQifCWB1Bp5Gblwfdcyi9kfW0LI

# ③ 验签：同一条消息 → true；换了内容 → false
rcli text verify -i msg.txt  -k keys/blake3.txt --sig "$SIG"     # true（实测）
rcli text verify -i test.rest -k keys/blake3.txt --sig "$SIG"    # false（实测）

# ④ ed25519：私钥签、公钥验
SIG2=$(rcli text sign -i msg.txt -k keys/ed25519.sk --format ed25519)
rcli text verify -i msg.txt -k keys/ed25519.pk --sig "$SIG2" --format ed25519   # true（实测）
```

关键事实：

- **签名结果统一用 `URL_SAFE_NO_PAD` base64 输出**（`process/text.rs`），所以签名串里出现 `-` `_` 而不是 `+` `/`，长度对 blake3 是 43 字符（32 字节）、ed25519 是 86 字符（64 字节）。
- **密钥文件就是原始字节**，不是 base64/PEM：`KeyLoader::load` = `fs::read` 拿 `Vec<u8>` 再 `try_new`。所以 `cat` 出来是乱码是正常的，别手改。
- 验签失败有两种表现：`blake3` 只比较哈希字节，不符就返回 `false`（**退出码 0**）；`ed25519` 用**错类型的钥匙**（例如拿私钥当公钥）会在加载阶段就失败，实测得 `Error: signature error`，**退出码 1**（不是 `false`）。脚本里要区分这两种。
- 密钥文件长度不对也是 Err 而不是 panic（已修）：实测 `-k` 指向一个 5 字节文件 → `Error: blake3 key must be exactly 32 bytes, got 5`，退出码 1（以前是切片越界 panic，退出码 101）。
- 缺 `--sig` 会被 clap 拦下（实测退出码 2）；`-i` 不给则**卡在等 stdin**（Windows Terminal 按 `Ctrl+Z`+Enter，Git Bash 按 `Ctrl+D`）。
- 设计上有意思的一点：`process/text.rs` 用四个小 trait 把"能力"拆开——`TextSign`/`TextVerify`/`KeyLoader`/`KeyGenerator`，于是 `Blake3` 同时实现签+验，`Ed25519Signer` 只实现签、`Ed25519Verifier` 只实现验。**类型层面就无法拿公钥去签名**，这是 Rust 用 trait 拆分表达约束的典型手法。

### 5.6 `rcli http` — 静态文件服务

```bash
rcli http serve                        # 默认 -d . -p 8080
rcli http serve -d assets -p 3000
RUST_LOG=info rcli http serve -d . -p 8080    # ⚠️ 想看访问日志必须设这个（见 §5.7）
```

服务里有**两条并行的实现路线**（同一目录，两种做法对照学习）：

| 路由 | 实现 | 特性 |
|---|---|---|
| `/{*path}` → `file_handler` | 手写 axum handler | 单文件返回文本；**目录会生成 HTML 索引页**（子目录优先、按名排序、URL/HTML 转义） |
| `/tower/{*path}` | `tower_http::services::ServeDir` | 现成方案：支持 `Range`、`ETag`、`Last-Modified`、precompressed(gzip/br/zstd/deflate) |

实测（`rcli http serve -d . -p 18099`）：

```
$ curl -i http://127.0.0.1:18099/fixtures/blake3.txt
200，32 B，content-type: text/plain; charset=utf-8

$ curl http://127.0.0.1:18099/src            # 目录索引（自己生成的 HTML）
<h1>Index of /src/</h1>
<li><a href="/src/cli/">cli/</a></li>        # 子目录在前且带结尾斜杠
<li><a href="/src/lib.rs">lib.rs</a></li>

$ curl -i http://127.0.0.1:18099/no-such.txt
404  File .\no-such.txt not found

$ curl -i -H "Accept-Encoding: gzip" http://127.0.0.1:18099/tower/Cargo.toml
200，accept-ranges: bytes，etag: "6ae7ea34..."，last-modified: ...   # tower 那条路的完整头部
```

`test.rest` 是给 VSCode REST Client 插件用的现成用例（仓库根目录）。
停止服务：`Ctrl+C`（本项目没装 tokio 的 `signal` feature，所以**不做优雅停机**，直接进程退出）。

### 5.7 日志：`RUST_LOG` 与一个容易踩的默认值

`main.rs` 只有一句 `tracing_subscriber::fmt::init();`。因为 `Cargo.toml` 给 `tracing-subscriber` 开了 **`env-filter` feature**，`fmt::init()` 的默认过滤器就变成了"读 `RUST_LOG`，**没设置时等于 `error`**"。

实测三种情形（`http serve` 里打的是 `info!`/`warn!`）：

| 命令 | 结果 |
|---|---|
| `rcli http serve -d . -p 18300` | **一条日志都没有**（info/warn 低于默认 error 阈值） |
| `RUST_LOG=info rcli http serve ...` | `INFO rcli::process::http_serve: Serving "." on port 18500`、`Read 1262 bytes`、`Listed dir .\src -> 365 bytes` |
| `RUST_LOG=warn ...`（代码里只有 info/warn 时） | 只有 `warn!` 那类会出来 |

- 日志默认写到 **stdout**（实测：`2>/dev/null` 之后日志仍在，`>file` 才收得到）。对 `http serve` 无所谓，但如果以后给 `genpass`/`csv` 加日志，就要考虑用 `with_writer(std::io::stderr)`，否则违反 §5.1 的 stdout 纪律。
- 把服务输出重定向到文件时，进程被 `kill` 掉可能看不到最后几行（未刷盘的缓冲丢了）；验证日志建议用 `RUST_LOG=info ... | tee` 或正常退出。
- `deny`/`fmt` 之外，`tracing-subscriber` 的 `env-filter` 还给了细粒度语法：`RUST_LOG=rcli::process::http_serve=debug`、`RUST_LOG=axum=info,rcli=trace`。

---

## 6. 怎么测试

### 6.1 现状：17 个用例，分布在 5 个文件（实测 `cargo test` 与 `cargo nextest run` 全绿）

| 文件 | 用例 | 测什么 | 备注 |
|---|---|---|---|
| `cli/mod.rs` | `test_verify_input_file` | 协议层：`-` 放行 / 存在 / 不存在 | `assert_eq!` 比整个 `Result` |
| `process/b64.rs` | `test_process_encode` | Cargo.toml 编码结果的**前缀字面量** | 以前只 `is_ok()` |
| `process/b64.rs` | `test_process_decode` | 夹具解出 `Hello, rcli base64!` 全文 | 以前只 `unwrap()` |
| `process/b64.rs` | `test_process_decode_rejects_wrong_alphabet` | 字母表用错必须 Err | 新增 |
| `process/gen_pass.rs` | `test_genpass_too_short_is_error_not_panic` | P0-1 回归：报 Err而不是 panic/静默超长 | 新增 |
| `process/gen_pass.rs` | `test_genpass_no_class_enabled_is_error` | 空字符池不能 panic | 新增 |
| `process/gen_pass.rs` | `test_genpass_returns_requested_length` | 长度精确等于请求值 | 新增 |
| `process/gen_pass.rs` | `test_genpass_covers_every_enabled_class` | 四类各至少出现一次 | 守"先各取一个 + shuffle" |
| `process/gen_pass.rs` | `test_genpass_minimal_length_is_exactly_required` | 边界：长度 1 报错、2 恰好通过 | 新增 |
| `process/text.rs` | `test_blake3_short_key_is_error_not_panic` | P0-2 回归：短密钥报 Err 而不是越界 panic | 新增 |
| `process/text.rs` | `test_blake3_sign_verify` | 签完自己验 → true | |
| `process/text.rs` | `test_blake3_verify_rejects_tampered_message` | 改一个字母 → false | 守"验不过是 false 而非报错"的语义 |
| `process/text.rs` | `test_ed25519_sign_verify` | 私钥签 → 公钥验 | |
| `process/text.rs` | `test_ed25519_verify_rejects_tampered_message` | 篡改 → false | 新增 |
| `process/http_serve.rs` | `test_file_handler` / `_not_found` / `_dir_index` | 不起服务直接 `await` handler，查状态码 + body | 强 |

值得学的两处写法：

- **同步业务测试**贴在被测文件里（`#[cfg(test)] mod tests`），直接调用函数传真实路径（`"Cargo.toml"`、`"fixtures/b64.txt"`），**不需要构造 argv**。
- **异步 handler 测试**用 `#[tokio::test]`，并且**不起服务**：直接 `file_handler(State(state), Path("Cargo.toml".into())).await` 拿到 `Response`，再用 `axum::body::to_bytes` 把 body 抽出来断言——测 HTTP 逻辑不等于要开端口。

### 6.2 跑测试

```bash
cargo test                                  # 单测 + 文档测试，一次跑完
cargo test -- --nocapture                   # 看到被测代码里 println!/eprintln! 的输出
cargo test cli::tests                       # 只跑模块前缀匹配的
cargo test -- --test-threads=1              # 串行（并发写同一文件时有用）
cargo test -- --exact test_process_encode   # 精确匹配一个
cargo llvm-cov nextest                      # 覆盖率（CI 里装了）
cargo nextest run                           # 逐测试进程，失败隔离、输出更清晰（推荐）
```

`cargo test` 会跑三组：`unittests src/lib.rs`（上面 17 个）、`unittests src/main.rs`（0 个）、`Doc-tests rcli`（0 个）。
注意 **doc 测试为 0**：库里没有 ```  ```rust,ignore ``` 形式的文档示例，将来在 `lib.rs` 写 `//! # 例子` 时它们会被真的编译执行。

### 6.3 手工验收清单（每条都可复制，含期望值）

```bash
cargo build --release && R=./target/release/rcli.exe

$R --version                        # → rcli 0.1.0
$R csv -i assets/juventus.csv -f json -o /tmp/o.json && head -2 /tmp/o.json   # → "[\n  {"
$R genpass -l 16 >/dev/null; echo $?   # → 0
$R genpass -l 3 --upper --lower --number --symbol; echo $?   # → 干净的 Error，退出码 1（不再是 panic）
printf 'abc' | $R base64 encode     # → YWJj
$R text verify -i msg.txt -k fixtures/blake3.txt \
    --sig 63P40ONwLHPxqPMTQQifCWB1Bp5Gblwfdcyi9kfW0LI      # → true
RUST_LOG=info $R http serve -d . -p 18080 &                # → 日志出现 "Serving"
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:18080/Cargo.toml    # → 200
```

### 6.4 断言强度：从"测不出对不对"到断言内容

现场事故（本文档写作期间确认）：`fixtures/b64.txt` 曾被一次 shell 重定向意外覆盖成别的内容，而 `cargo test` **依然全 passed**——
因为当时 `test_process_decode` 只写了 `process_decode(input, format).unwrap()`，`test_process_encode` 只写了 `is_ok()`。
只测"有没有崩"、不测"对不对"，夹具烂了也不知道。

现在已经改成断言内容（实测通过）：

```rust
// src/process/b64.rs —— 让 fixture 与期望值成对出现，断言内容而不是断言"没报错"
#[test]
fn test_process_decode() {
    let got = process_decode("fixtures/b64.txt", Base64Format::UrlSafe).unwrap();
    assert_eq!(String::from_utf8(got).unwrap(), "Hello, rcli base64!");
}
```

剩下的同类缺口：`cli/*.rs` 里的 `execute()` 仍然在内部 `println!`，所以"stdout 内容对不对"至今只能靠集成测试（§6.5 方案 B）戒掉。

### 6.5 集成测试（还没有，这是推荐的下一步）

因为 `src/lib.rs` 这个库 target 存在，集成测试有两种写法：

```rust
// tests/cli.rs —— A. 直接调库函数（快、可断言返回值）
use rcli::{Base64Format, process_encode};
#[test] fn encode_is_stable() {
    assert_eq!(process_encode("fixtures/b64.txt", Base64Format::Standard).is_ok(), true);
}

// tests/e2e.rs —— B. 起真进程验 stdout/stderr/退出码（才真的覆盖 clap + enum_dispatch 链）
// 需要 cargo add --dev assert_cmd predicates
use assert_cmd::Command;
#[test] fn genpass_prints_only_password_on_stdout() {
    let out = Command::cargo_bin("rcli").unwrap().args(["genpass","-l","12"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stderr).trim().starts_with("Password strength:"), true);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim().chars().count(), 12);
}
```

方案 B 是唯一能覆盖"§3 那条分发链 + stdout/stderr 分工"的手段，建议第一个集成测试就写它。

---

## 7. 工程化关卡：提交前和 CI 各跑什么

这些文件不影响功能，但它们是"课程项目"和"可发布工具"的分界线。

### 7.1 `.pre-commit-config.yaml`（本地提交前）

| hook | 命令 | 作用 |
|---|---|---|
| pre-commit 官方 9 项 | — | 字节序/BOM、大小写冲突、合并冲突残留、悬空软链、YAML 合法性、文件末尾换行、行尾风格、尾随空格 |
| black | — | Python 格式化（模板带来的，本项目没有 .py，永远空跑） |
| cargo-fmt | `cargo fmt -- --check` | 风格统一，让 diff 只反映逻辑变化 |
| cargo-deny | `cargo deny check -d` | 依赖许可证 / 已知漏洞 |
| typos | `typos` | 拼写检查，`pass_filenames: false` |
| cargo-check | `cargo check --all` | 类型检查 |
| cargo-clippy | `cargo clippy --all-targets --all-features --tests --benches -- -D warnings` | 把 lint **从提示升级为错误** |
| cargo-test | `cargo nextest run --all-features --no-tests pass` | 逐测试进程执行 |

注意两点：cargo 类 hook 是 `bash -c '...'` 包起来的，且 `pass_filenames: false`（cargo 作用于整个包，不是单个文件）；
本机 `pre-commit` 未安装，所以这套关卡目前**只在 CI 里真的跑过**。

### 7.2 `.github/workflows/build.yml`（push/PR 到 main、打 tag 时）

`checkout(fetch-depth: 0)` → 装 stable toolchain + `cargo-llvm-cov` + `nextest` → `rust-cache`
→ `cargo fmt --check` → `cargo check --all` → `cargo clippy ... -D warnings` → `cargo nextest run --all-features`
→ **打 tag 时**：`git-cliff` 生成 CHANGES.md 作为 GitHub Release 的 body。

`fetch-depth: 0` 是 cliff 的硬要求（要读全部历史才能分类 commit）；这就是"提交信息必须按 conventional commits 写"的直接原因。

配套：`cliff.toml` 用 `$REPO` 替换出仓库 URL、`tag_pattern = "v[0-9].*"`（所以 `v1-8-genpass` 这类 tag 会被认作版本）；
`CHANGELOG.md` 由 `git-cliff` 生成，不要手改。

### 7.3 当前每一关的实测红绿灯

本轮把红的几关都修绿了（以下全部实测）：

| 关卡 | 状态 | 说明 |
|---|---|---|
| `cargo check --all-targets` | 🟢 | 零 warning |
| `cargo test` / `cargo nextest run` | 🟢 | 17 passed |
| `cargo fmt -- --check` | 🟢 | 已执行全量 `cargo fmt`（原先 12 个文件 55 处 diff）。**这类纯格式变更建议单独一个 commit** |
| `cargo clippy ... -D warnings` | 🟢 | 已把 `STANDARD.decode(&buf)` 改成 `decode(buf)`（原先 2 个 `needless_borrows_for_generic_args`） |
| `cargo deny check licenses` | 🟢 | `deny.toml` 加了 `"Unicode-3.0"`（`unicode-ident v1.0.27` 起的新许可证；旧的 `Unicode-DFS-2016` 继续留着） |
| `cargo deny check bans` / `sources` | 🟢 | bans 只报 multiple-versions（配置为 `warn`） |
| `cargo deny check advisories` | ⚪ 未测 | 需要联网拉 advisory-db |
| `typos` | 🟢 | 公开 trait 全局改名（旧名比新名少一个字母 `u`）+ 一个测试函数名拼写修正，两处都是把错词直接改对，**没有往 `_typos.toml` 里加白名单** |
| `pre-commit` | ⚪ 未装 | 本机 `pre-commit: command not found`，这套关卡只有 CI 跑得到 |

> 本轮修动的具体内容列在 §8.1；还开着的问题见 §8.2。

### 7.4 版本发布流程

```bash
git tag v1-13-enum-dispatch        # tag 名要匹配 cliff 的 v[0-9].*
git push origin v1-13-enum-dispatch # 触发 build.yml 的 release 分支
```

提交信息格式（cliff 按前缀分类）：`feat:` / `fix:` / `chore:` / `refactor:` / `docs:`。
现有 tag：`v1-8-genpass`、`v1-11-http-static-server`、`v1-12-tower-http`。

---

## 8. 问题清单：已修与待修

### 8.1 本轮已修（全部附回归测试）

| 原问题 | 修复前的实测症状 | 修法 | 守它的测试 |
|---|---|---|---|
| **P0-1** `gen_pass.rs` 的 `length - password.len()` 是 u8 减法，长度小于启用字符类数就下溢 | debug `attempt to subtract with overflow`（退出码 101）；release **静默输出 259 字符** | 函数开头两段 `anyhow::ensure!`：先要求"至少启用一个字符类"，再要求 `length >= 启用类数` | `test_genpass_too_short_is_error_not_panic` 等 5 个 |
| **P0-2** `Blake3::try_new` 写 `&key[..32]`，密钥文件不足 32 字节就切片越界 | `range end index 32 out of range for slice of length 5`（退出码 101） | 改 `key.try_into()` + `map_err` 出带实际长度的文案，与 `Ed25519Verifier` 行为对齐 | `test_blake3_short_key_is_error_not_panic` |
| **P1-1** `cli/base64.rs` 的 decode 用 `println!("{:?}", ret)` | 输出 `[72, 101, 108, ...]` 而不是原文 | `String::from_utf8(ret)?` 后打印文本；"解出来不是 UTF-8"变成可见错误 | `test_process_decode`（已改成断言全文） |
| **P1-3** 三道关卡红 | fmt 12 文件 55 处 / clippy 2 条 / deny licenses | `cargo fmt` 全量归一、`decode(&buf)` → `decode(buf)`、`deny.toml` 加 `"Unicode-3.0"` | 关卡本身（§7.3 现全绿） |
| **P2 拼写** | trait 旧名比新名少一个字母 `u`（8 文件 21 处）、一个测试函数名把 `verify` 拼错、`on prot`、`not found: `、`a director over http` | 公开 API 全局改名 `CmdExecutor`；测试改名 `..._verify`；三处文案修正 | `typos` 关卡转绿（未加白名单） |
| **P2 测试结构** | `process/text.rs` 的两个 `#[test]` 裸在模块顶层，没有 `#[cfg(test)]` | 收进 `#[cfg(test)] mod tests`，与 `http_serve.rs` 一致 | — |

两处修复的取舍值得记一笔：

- **P0-1 为什么不用 `saturating_sub`**：那只是把错误结果换个姿势继续输出（得到一个长度不对的密码）。宁可返回 `Err` 让用户看见。
- **P0-2 为什么不给 `Blake3` 加 `#[derive(Debug)]`**：测试里用 `unwrap_err()` 需要 `Ok` 侧也 `Debug`，但**不该为了让测试好写而扩大公开 API 面**；改成 `.err().unwrap()` 即可（`Option::unwrap` 只要求 `E: Debug`，`anyhow::Error` 满足）。
- **为什么同一个项目里两类错误码不一致是个信号**：blake3 短密钥报 101、ed25519 错钥报 1——同一类失败应该有同一类退出码。

### 8.2 还开着的

#### P1 `csv` 的 `--delimiter` / `--header` 是"空头承诺"

`cli/csv.rs` 声明了这两个参数，但 `CsvOpts::execute` 只往下传 `input/output/format` ——
**用户能传，传了毫无作用**。这类接口比"没实现"更危险。要么接线（改用 `csv::ReaderBuilder::new().delimiter(...).has_headers(...)`），要么先从 CLI 删掉。

#### P2 一致性、错误信息与其他

- `verify_file` 的错误信息是不带文件名的固定串 `"File does not exist"`，用户不知道是哪个参数错了。返回类型 `Result<String, &'static str>` 决定它没法带动态内容，要改成 `String`/`anyhow::Error`。
- `csv_convert.rs` 的 `Vec::with_capacity(128)` 是魔法数：它只是预分配提示（超过 128 行照样能跑），但读代码的人会以为是上限。
- `lib.rs` 的 `pub use process::*;` / `pub use utils::*;` 是 glob 再导出：省事，但**公开 API 面看不见**，将来新增同名函数会静默改变门面。
- `Blake3::verify` 用 `hash == sig` 直接比较，**不是常数时间比较**（`constant_time_eq` 或 `subtle::ConstantTimeEq`）。对这个学习项目不算 bug，但 keyed hash 的验证本应抵抗计时侧信道——值得写进注释当反面教材。
- `process_encode` / `process_decode` 一次性 `read_to_end`，大文件全进内存。base64 支持流式（0.23 的 `base64::write::EncoderWriter` / `base64::read::DecoderReader`；注意 `Engine` trait 上**没有** `encode_reader` 这种方法）。
- `file_handler` 用 `tokio::fs::read_to_string` 读文件：**二进制文件会因非 UTF-8 直接 500**。实测同一个 `fixtures/ed25519.sk`：
  `GET /fixtures/ed25519.sk` → `500` + body `stream did not contain valid UTF-8`；`GET /tower/fixtures/ed25519.sk` → `200`。
  而且大文件全进内存。`/tower` 那条路（`ServeDir`）才是对的——**这正是两条路并存的价值：一个把问题暴露出来，一个把问题解决掉**。
- 目录遍历只列第一级（`list_dir` 的注释已说明），没有 `..` 链接，没有排序开关 —— 属于"够用"的实现。

#### P3 模板残留

- `CHANGELOG.md` 的 `[unreleased]` 段仍指向 `github.com/tyrchen/qdrant-lib`（上游模板带来的）。`cliff.toml` 里的 URL 已经改对（`bakenray/01-rcli`），但历史 CHANGELOG 没重新生成：`git-cliff -o CHANGELOG.md`。
- `Cargo.toml` 的 `license` 字段挂了 8 个 `OR`；`OR` 的语义是"用户可以任选一个许可证"，一般写 `"MIT OR Apache-2.0"` 就够。
- `deny.toml` 的 `[bans]`/`[sources]` 大部分还是模板注释状态，等于"跑了但没真正约束"。
- `build.yml` 的 `platform: ["ubuntu:22.04"]` 是容器 runner（沿用作者环境），Windows 本地跑不动那些 `bash -c` hook。

### 8.3 建议的练习顺序（按学习收益）

1. 接上 `--delimiter` / `--header`，练 `csv::ReaderBuilder`（§8.2 唯一的 P1）。
2. 把 `process_*` 改成**返回 `String` 而不是内部 `println!`**，让打印回到 `cli/*.rs` 的 `execute` 里 —— 业务函数立刻变得可断言（§6.4 说的同类缺口）。
3. 加 `tests/` 集成测试（`assert_cmd` + `predicates`），第一次真正端到端验证 stdout/stderr/退出码（§6.5 方案 B）。
4. 把 `file_handler` 的 `read_to_string` 换成流式 `ServeFile` 或 `Body::from_stream`，二进制文件就不会 500 了。
5. 给 `blake3` 验签换成常数时间比较（`constant_time_eq`），并把 `Engine` 导入风格统一。
6. 发布前：`cargo deny check`（含 advisories）、多平台 matrix（`windows-latest` + `macos` + `ubuntu`）、`cargo-dist` 生成二进制包；顺手重生成 CHANGELOG。

---

## 9. 新增一个子命令的标准步骤（照抄即可）

假设要加 `rcli hash -i file -a sha256`：

1. **协议层** `src/cli/hash.rs`：

```rust
use clap::{Parser, ValueEnum};
use crate::CmdExecutor;
use super::verify_file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HashAlg { Sha256, Sha512 }

#[derive(Debug, Parser)]
pub struct HashOpts {
    #[arg(short, long, value_parser = verify_file)]
    pub input: String,
    #[arg(short, long, value_enum, default_value = "sha256")]
    pub alg: HashAlg,
}

impl CmdExecutor for HashOpts {                 // 叶子实现写在同一个文件里
    async fn execute(self) -> anyhow::Result<()> {
        println!("{}", crate::process_hash(&self.input, self.alg)?);
        Ok(())
    }
}
```

2. **业务层** `src/process/hash.rs`：`pub fn process_hash(input: &str, alg: HashAlg) -> anyhow::Result<String>`，用 `crate::get_reader(input)?` 取输入。
3. `src/process/mod.rs` 加两行：`mod hash;` + `pub use hash::process_hash;`。
4. **挂到 enum**：`src/cli/mod.rs` 里 `mod hash;` → `pub use self::hash::{HashOpts, HashAlg};` → 给 `SubCommand` 加 variant `Hash(HashOpts)`（`#[enum_dispatch(CmdExecutor)]` **不用改**，宏按 variant 自动生成一个 match 分支）。
5. `src/lib.rs` 的门面按需补 `pub use cli::HashAlg;`（**只在你希望外部当库用时**才导出）。
6. 子命令还有下一级时（`hash file` / `hash dir`）：给 `HashSubCommand` 也加 `#[enum_dispatch(CmdExecutor)]`，并删掉任何手写的 `impl CmdExecutor for HashSubCommand`。

**两条最容易忘的**：
- 分发是宏生成的，**忘删手写 impl 会 E0119 冲突**；
- `#[enum_dispatch]` 标注的 trait 必须留在 `mod cli;` **之前**（§3.2 第 1 条），否则错误信息会非常迷惑（`cannot find type ... exists but is inaccessible`）。

编译验证：`cargo check --all-targets` → `cargo run -- hash -i Cargo.toml` → `cargo test`。

---

## 10. 数据文件与夹具

| 路径 | 是什么 | 谁在用 |
|---|---|---|
| `assets/juventus.csv` | 尤文球员名单（来自 The-Football-Data），列名带空格、值全是文本、有 `Apr 18, 1990 (29)` 这种混合值 | `csv` 手工试数据（**测试不依赖它**） |
| `fixtures/b64.txt` | `Hello, rcli base64!` 的 **url-safe 无填充** base64：`SGVsbG8sIHJjbGkgYmFzZTY0IQ` | `test_process_decode` |
| `fixtures/blake3.txt` | 32 字节 blake3 共享密钥（原始字节，`cat` 出来是乱码属正常） | `test_blake3_sign_verify`、手工验签 |
| `fixtures/ed25519.sk` / `.pk` | 32 字节 ed25519 私钥 / 公钥（原始字节，非 PEM） | `test_ed25519_sign_verify`、手工验签 |
| `fixtures/index.html` | 一小段 HTML | `http serve -d .` 时的静态页样本 |
| `msg.txt` | 内容 `hello!`，被签名的目标 | `text sign/verify` 手工闭环 |
| `test.rest` | REST Client 用例（`GET http://localhost:8080/fixtures/blake3.txt` 等） | 起 `http serve` 后在 VSCode 里点着测 |
| `output.{json,yaml,toml}` | `csv` 的默认产物 | 已在 `.gitignore`（连同 `/target`） |

**约定**：`assets/` 给人手工试，`fixtures/` 给测试用。夹具一旦改动会让对应测试失真——
`test_process_decode` 现在**不校验内容**，所以夹具被改也测不出来（§6.4 讲过的事故）。

---

## 11. 贯穿全项目的心智模型

1. **CLI 三段式**：解析（协议）→ 分发（trait/enum）→ 处理（业务）。三段都能独立替换、独立测试。
2. **enum 是穷尽的状态机**：`SubCommand`/`OutputFormat`/`Base64Format`/`TextSignFormat` 都不带逻辑，逻辑全在实现和 `match` 里。新增 variant 时编译器会把所有没改到的地方列出来（穷尽性检查）；`enum_dispatch` 更进一步，连那个 `match` 都替你生成。
3. **`?` 不只是提前返回**：它调用 `From::from` 把 `io::Error`/`DecodeError`/`Utf8Error`/`TryFromSliceError` 统一转成 `anyhow::Error`。所以 `reader.read_to_end()?` 和 `STANDARD.decode()?` 能待在同一个函数里——**anyhow 用类型擦除换来了跨错误类型的简洁**。
4. **`anyhow` 给应用，`thiserror` 给库**：CLI 只需要"把错误说给人听"，所以全项目 `anyhow` 是对的；哪天真把 `process_*` 发布成库、调用方需要 `match` 具体错误码，再换 `thiserror`。
5. **`-` 代表 stdin 是 Unix 惯例**（`cat -`、`curl`）：`verify_file` 放行 `"-"` + `get_reader` 翻译 `"-"` → `stdin`，两处配合才让 `printf abc | rcli base64 encode` 成立。
6. **trait 方法必须"trait 在作用域"才可见**：`Engine as _`（`use base64::Engine`）就是为了 `STANDARD.encode()` 能编译；同理 `main.rs` 必须 `use rcli::CmdExecutor` 才能 `.execute()`。`as _` = "只要它的方法，不要它的名字"。
7. **`Box<dyn Read>` vs 泛型 `impl Read`**：`get_reader` 必须在运行时按 `"-"` 切换 `Stdin`/`File`，两者没有共同类型 → 只能 `Box<dyn Trait>`（trait object 是 DST，必须装箱）；`TextVerify::verify(&self, reader: impl Read, ...)` 用泛型（编译期单态化，更快，但调用点类型必须固定）。**同一个项目里两种都存在，正好对照**。
8. **字节 vs 文本的边界意识**：`gen_pass` 全程 `Vec<u8>` 最后 `from_utf8`；`b64` encode 收字节、decode 吐文本；`text` 的签名是字节、输出前统一 base64 编码成文本。**边界在哪断裂，错误就在哪冒出来**。
9. **`clap` derive 是"声明式 + 值解析"**：`#[arg(value_parser = ..., default_value = ..., value_enum)]` 让"字符串 → 领域类型"发生在进入结构体之前，所以业务函数**永远不用再判断参数合法性**（对比 Python 里函数开头那一堆 `if args.x is None`）。
10. **静态优先，异步只在必要处**：5 个子命令里只有 `http serve` 真异步，但分发契约是 `async fn`——静态分发的代价是"每个分支都被实例化"，同步实现写成 `async fn` 不产生额外开销。
11. **测试写在离代码最近的地方**：单元 `#[cfg(test)]` 贴在同文件；`fixtures/` 是给测试的数据；跨进程的端到端验证留给 `tests/`（还没建，见 §6.5）。

---

## 12. 一句话总结

`csv` 讲 **serde 的数据模型**，`genpass` 讲 **字节与随机 + Unix 的 stdout/stderr 纪律**，`base64` 讲 **trait object 与二进制↔文本的边界**，`text` 讲 **用 trait 拆分能力来表达安全约束**，`http` 讲 **axum 路由与"手写 vs 现成"的对照**，`clap` 层讲 **把字符串升级成类型**，`lib.rs`/`main.rs` 的切分讲 **可测试、可复用代码长什么样**，`enum_dispatch` 讲 **怎样把机械的 match 交给宏**，那一堆配置文件讲 **一个能给别人用的工具除了功能还需要什么**。

数据流走通只是第一课；§8 里那些"能跑但有味道"的地方，才是 Rust 真正在约束你的东西。

## License

以 `Cargo.toml` 的 `license` 字段为准，目前是一长串 `OR`：
`MIT OR Apache-2.0 OR Unicode-DFS-2016 OR MPL-2.0 OR BSD-2-Clause OR BSD-3-Clause OR ISC OR CC0-1.0`。

`OR` 的语义是"使用者可任选其一"，且仓库里**没有 LICENSE 文件正文**。真要发布前建议收敛成 `MIT OR Apache-2.0` 并补上正文（见 §8 P3）。
