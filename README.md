# Rcli

一个用 Rust 写的命令行工具箱：**CSV 转格式**、**随机密码生成**、**Base64 编解码**。

> 这份 README 的重点不是"这个工具有哪些参数"，而是**每一块代码为什么要这么写**——
> 帮你把跟着课程写下的代码，还原成一套能说清楚的设计逻辑。
> 文中所有"实测"的输出都是在本仓库当前代码上真实跑出来的。

---

## 1. 三十秒看懂全貌

```
命令行 argv
   │
   ▼  clap(derive) 解析 + 校验          ← src/cli/*     "用户能说什么"
 Opts { cmd: SubCommand }
   │
   ▼  main.rs 只做一次 match 分发        ← src/main.rs   "薄壳"
 process_csv / process_genpass / process_encode / process_decode
   │
   ▼  真正的业务逻辑 + IO               ← src/process/*  "做了什么"
 stdout / stderr / 输出文件
```

一句话总结整个项目的骨架：**`cli/` 负责把字符串变成有类型的东西，`process/` 负责用这些东西干活，`main.rs` 只负责把两边对接起来。**

三个子命令各自"在练什么"：

| 子命令 | 表面功能 | 实际是在练 |
|---|---|---|
| `csv` | CSV → JSON/YAML/TOML | serde 的**数据模型 vs 序列化表示**、泛型序列化、多格式输出 |
| `genpass` | 生成随机密码 | `&[u8]` 字节、迭代器/随机 trait、**stdout 与 stderr 的分工** |
| `base64` | 编解码文件内容 | **二进制 ↔ 文本的边界**、`Box<dyn Read>` trait object、stdin 约定 `-` |

---

## 2. 目录结构，以及每个文件存在的理由

```
src/
├── main.rs              # bin target：只有 Opts::parse() + 一个 match，不写业务
├── lib.rs               # lib target：crate 的"公开门面"，只做 mod 声明 + pub use 再导出
├── cli/                 # 协议层：命令行长什么样
│   ├── mod.rs           #   Opts / SubCommand 枚举 + 公共校验函数 verify_file
│   ├── csv.rs           #   CsvOpts + OutputFormat（含 Display/FromStr 双向转换）
│   ├── genpass.rs       #   GenPassOpts（纯默认值声明）
│   └── base64.rs        #   Base64SubCommand + Base64Encode/DecodeOpts + Base64Format
└── process/             # 业务层：拿到类型后的实际处理
    ├── mod.rs           #   只导出 process_* 四个函数
    ├── csv_convert.rs   #   读 CSV → serde_json::Value → 三种格式文本
    ├── gen_pass.rs      #   字符池 → 保证覆盖 → 洗牌 → 强度评估
    └── b64.rs           #   get_reader 抽象输入源 → base64 Engine 编解码

assets/juventus.csv      # 练习数据（球队名单，字段类型五花八门）
fixtures/b64.txt         # 测试夹具：一段合法的 url-safe 无填充 base64
texitures/               # ⚠️ 空目录，疑似 textures 的手误（见 §8）
output.{json,yaml,toml}  # csv 子命令的产物，已在 .gitignore 里
Cargo.toml / Cargo.lock  # 依赖与锁版本
cliff.toml               # git-cliff：从 commit 生成 CHANGELOG
deny.toml                # cargo-deny：许可证 + 已知漏洞
_typos.toml              # typos：拼写检查
.pre-commit-config.yaml  # 提交前六道关卡
.github/workflows/build.yml  # CI + 打 tag 发 release
```

### 2.1 为什么要同时有 `main.rs` 和 `lib.rs`（一个包两个 target）

这是 Rust CLI 项目最标准也最容易被忽略的做法。`Cargo.toml` 里 `[package]` 下没有任何 `[[bin]]`，cargo 会**自动**发现：

- `src/main.rs` → 可执行程序 `rcli`；
- `src/lib.rs` → 同名库 `rcli`，可以被 `use rcli::process_csv` 这样引用。

好处有三个，都很实在：

1. **集成测试能 `use rcli::...`**。`tests/` 目录下的集成测试只会链接**库 target**，bin crate 里的模块对外不可见。只有拆出 `lib.rs`，测试才能直接调用 `process_csv(...)` 断言返回值；否则只能用 `assert_cmd` 去 spawn 进程、再解析 stdout 字符串。
2. **`main.rs` 保持薄**。`main` 只做 `Opts::parse()` + 一次 `match`（[src/main.rs](file:///d:/codes/01-rcli/src/main.rs#L12-L44)），业务函数是 `pub fn`，可以被别的 crate 直接当库调用——你的 CLI 顺手变成了一个可复用 library。
3. **`pub` 边界由编译器把关**。哪些类型是门面（走 `lib.rs` 的 `pub use`）、哪些是内部实现（`mod cli;` 保持私有），一眼能看出来；这也解释了为什么 `src/process/csv_convert.rs` 能写 `use crate::cli::OutputFormat;`（同 crate 内私有路径可用），而 `main.rs` 必须写 `use rcli::Base64Format;`（外部只能走门面）。

> 补一句：bin crate 里当然也能写 `#[cfg(test)]`（`cargo test` 会跑），所以"分层 + lib 门面"的价值主要是**复用、集成测试和可见性边界**，而不是"不然没法测"。

代价是要维护"门面"：[src/lib.rs](file:///d:/codes/01-rcli/src/lib.rs) 用 `pub use` 把内部模块的类型重新导出到 crate 根：

```rust
mod cli;
mod process;

pub use cli::{Opts, SubCommand, Base64Format, Base64SubCommand};
pub use process::*;
```

> **踩过的坑（本次修复）**：这行原本写成 `pub use cli::{ Opts, SubCommand, Base64Format, Base64Format };`，同一个名字出现两次 → E0252；而 `src/cli/base64.rs` 整个文件缺失 → E0583，进而让 `src/process/b64.rs` 里的 `use crate::Base64Format;` 报 E0432。
> **连锁报错的定位顺序**：永远先修最靠前的"模块级"错误（E0583 文件不存在、E0432 名字找不到），后面一串下游错误往往是同一个根因。

### 2.2 为什么 `cli/` 和 `process/` 要分成两层

看 `process` 层的函数签名：

```rust
pub fn process_csv(input: &str, output: String, format: OutputFormat) -> Result<()>
pub fn process_encode(input: &str, format: Base64Format) -> anyhow::Result<()>
```

它们**只接 `&str` 和自己理解的 enum，完全不认识 `CsvOpts` / `Base64EncodeOpts` 这些 clap 结构体**。这就是分层的价值：

- 换 CLI 框架（clap → argh）不用动业务代码；
- 业务函数可以脱离命令行独立测试（测试里直接传 `"Cargo.toml"`，不用构造 argv）；
- 反过来，业务层新增能力也不需要改 `main` 之外的东西。

### 2.3 为什么 `cli/mod.rs` 里到处用 `self::`

[src/cli/mod.rs#L7-L9](file:///d:/codes/01-rcli/src/cli/mod.rs#L7-L9) 的注释已经写了原因，但值得展开：

```rust
use self::{csv::CsvOpts, genpass::GenPassOpts};
pub use self::{csv::OutputFormat, base64::{Base64SubCommand, Base64Format}};
```

crate 里同时存在 `cli::csv` 和 `process::csv_convert`、`cli::base64`（模块）和外部 crate `base64`。Rust 2018+ 允许 `use csv::...` 这种写法同时可能命中"本模块的 csv"或"外部 crate csv"，编译器会报歧义。**`self::` = 明确说"我要的是当前模块下的那个"**，`crate::` = 从 crate 根开始找，`super::` = 上一层模块。

同理，[src/cli/csv.rs#L3](file:///d:/codes/01-rcli/src/cli/csv.rs#L3) 的 `use super::verify_file;` 是从父模块（`cli/mod.rs`）拿那个公共校验函数——**`verify_file` 只写了一次，csv 和 base64 两个子命令都在用**，这就是"参数校验属于协议层"的落点。

---

## 3. 一次运行的完整生命周期

以 `cargo run -- base64 encode -i Cargo.toml -f url-safe` 为例：

1. **`Opts::parse()`**（[src/main.rs#L13](file:///d:/codes/01-rcli/src/main.rs#L13)）— `#[derive(Parser)]` 生成的代码读 `std::env::args`，按 `Opts`/`SubCommand`/`Base64SubCommand` 的字段和属性匹配。
2. **`value_parser = verify_file`**（[src/cli/base64.rs#L15-L16](file:///d:/codes/01-rcli/src/cli/base64.rs#L15-L16)）— 在字符串进入结构体之前做一次"存在性检查"，顺便把 `"-"` 当合法值放行（原理见 §5 第 5 条）。
3. **`-f url-safe` → `Base64Format::UrlSafe`** — 因为 `Base64Format` derive 了 `ValueEnum`，clap 自动生成 kebab-case 取值表；类型转换在**解析阶段**完成，业务层拿到的一定是合法 enum。
4. **`main` 的 match**（[src/main.rs#L34-L41](file:///d:/codes/01-rcli/src/main.rs#L34-L41)）— 嵌套 match：外层挑 `SubCommand::Base64`，内层挑 `Base64SubCommand::Encode`。
5. **`process_encode`**（[src/process/b64.rs#L6-L23](file:///d:/codes/01-rcli/src/process/b64.rs#L6-L23)）— `get_reader(input)?` 拿到一个 `Box<dyn Read>` → `read_to_end` 读进 `Vec<u8>` → match format 选引擎 → `println!`。
6. 任何一步失败都用 `?` 冒泡到 `main` 的 `-> anyhow::Result<()>`，由 Rust 的 `Termination` 把错误打到 stderr 并以非 0 退出码结束。

---

## 4. 三个子命令逐个拆解：为什么要写它、为什么要这么写

### 4.1 `csv` — 不是"为了转格式"，是为了理解**数据模型**

[src/process/csv_convert.rs#L21-L48](file:///d:/codes/01-rcli/src/process/csv_convert.rs#L21-L48)

**演进过程就是这门课的核心线索**（对照 git 历史：`feature: 处理csv->json` → `fix: 支持输出格式：json、toml、yaml`）：

最初一定是定义一个强类型结构去反序列化，也就是现在[留下来的 `struct Player`](file:///d:/codes/01-rcli/src/process/csv_convert.rs#L9-L19)：

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct Player { name: String, position: String, ... kit: u8 }
```

问题立刻出现：**这个函数凭什么假设输入一定是"球员名单"？** 列名换了、多一列少一列、`Kit Number` 里混进空值，`reader.deserialize::<Player>()` 就全线崩。所以现在的实现换成了一套**与 schema 无关的通用模型**：

```rust
let headers = reader.headers()?.clone();          // 第一行当 key
... headers.iter().zip(record.iter())             // zip 把两行配对
    .collect::<Map<String, Value>>();             // 得到 serde_json::Value::Object
```

> 于是 `Player` 就成了 `warning: struct Player is never constructed`——**它是上一版实现留下的尸体**，现在没有任何代码用它（见 §8）。

**关键心智**：serde 的 `Serialize`/`Deserialize` 不是"JSON 的解析器"，而是**一套中间数据模型**（`Value`）的接口。同一份 `Vec<Value>` 交给不同后端就得到不同文本：

```rust
OutputFormat::Json => serde_json::to_string_pretty(&ret)?,
OutputFormat::Yaml => serde_yaml::to_string(&ret)?,
OutputFormat::Toml => toml::to_string(&json!({ "rows": ret }))?,
```

这也解释了 `OutputFormat` 这个 enum 为什么"什么逻辑都没有"——**它只是一个穷尽分支的开关**，加一种格式 = 加一个 variant + 加一个 match 臂，编译器会告诉你所有没改到的地方（穷尽性检查）。

`csv.rs` 里那三块 impl 也是同一件事的不同方向（[src/cli/csv.rs#L30-L60](file:///d:/codes/01-rcli/src/cli/csv.rs#L30-L60)）：

| 实现 | 作用 | 谁在用 |
|---|---|---|
| `From<OutputFormat> for &'static str` | 枚举 → 文件名后缀 | `main.rs` 拼默认输出名 `output.{format}` |
| `FromStr for OutputFormat` | 字符串 → 枚举，非法值返回 Err | 用户 `-f xxx` |
| `fmt::Display` | 枚举 → 可打印 | `format!("output.{}", csv_opts.format)` |
| `parse_format` | 把 `FromStr` 包成 clap 要的 `value_parser` | `#[arg(value_parser = ...)]` |

**为什么 `-o` 不给默认值而是 `Option<String>`**：因为默认文件名依赖 `format`（`output.json` / `output.yaml`），而 clap 的属性里没法表达"依赖别的字段"，所以只能在 `main.rs` 里运行时决定（[src/main.rs#L17-L21](file:///d:/codes/01-rcli/src/main.rs#L17-L21)）。这是 CLI 设计里很常见的一类妥协。

### 4.2 `genpass` — 一个"看似简单"的算法题 + Unix 输出礼仪

[src/process/gen_pass.rs](file:///d:/codes/01-rcli/src/process/gen_pass.rs)

字符池用**字节字符串常量**：

```rust
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
```

为什么是 `&[u8]` 而不是 `&str`：① 密码生成的本质是**字节级随机**，最后才 `String::from_utf8` 转文本；② `&[u8]` 可以直接 `extend_from_slice` 拼成一个大池子，也能 `push` 进 `Vec<u8>`，全程避免字符/字节混淆。注释里那句"`const` 必须指定类型"是语法点：`const` 不像 `let` 能推导，必须写全 `&[u8]`。

算法三段式，每段都有理由：

1. **每个启用的字符类先各取一个**（[L21-L36](file:///d:/codes/01-rcli/src/process/gen_pass.rs#L21-L36)）—— 这是"保证约束成立"的标准做法。如果只从大池子里随机 `length` 次，理论上可能出一个没有符号的"含符号密码"。
2. **剩下的位数从合并池随机填充**（[L38-L43](file:///d:/codes/01-rcli/src/process/gen_pass.rs#L38-L43)）。
3. **`password.shuffle(&mut rng)`** —— 必需！否则前四位一定"大写、小写、数字、符号"，位置规律直接泄露。

`expect("UPPER won't be empty")` 这种写法是刻意的：**`choose()` 返回 `Option`，空切片才返回 `None`，而常量池不可能为空**。这里不适合 `?`（`Option` 不在 `anyhow::Result` 的路径上），也不该 `unwrap()`（没说明理由）；`expect + 理由` 是社区惯例。

最后两行是 Unix 礼仪的重点（[L49-L53](file:///d:/codes/01-rcli/src/process/gen_pass.rs#L49-L53)）：

```rust
println!("{}", password);                  // stdout：结果，能被管道/命令替换消费
eprintln!("Password strength: {}", ...);   // stderr：给人看的元信息
```

这样 `pass=$(rcli genpass)` 只会拿到密码本身，强度提示不会污染变量。**"结果走 stdout，日志/提示走 stderr"** 是这条命令能进脚本的前提。

### 4.3 `base64` — 全项目信息密度最高的一个文件

[src/process/b64.rs](file:///d:/codes/01-rcli/src/process/b64.rs)

**① 第一行为什么写 `Engine as _`**

```rust
use base64::{engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD}, Engine as _};
```

`STANDARD.encode(...)` 看着像方法，其实 **`encode` 是 trait `Engine` 的方法**。Rust 规定：**要调用 trait 方法，必须把这个 trait 引入作用域**，否则报 `no method named 'encode' found for struct GeneralPurpose`——这是 Rust 新手最常见的"我明明看着它有这个方法"困惑源。而 `Engine` 这个名字你并不打算直接用，所以写成 `as _`：**只要它的能力，不要它的名字**（避免和别的 crate 的同名类型冲突）。写成 `use base64::Engine;` 也完全可以——通过方法调用用到 trait 就算"已使用"，不会报 unused import；`as _` 只是把"我不需要这个名字"的意图写出来。

**② encode 和 decode 为什么输入类型不一样**

| | 缓冲类型 | 为什么 |
|---|---|---|
| `process_encode` | `Vec<u8>` + `read_to_end` | 被编码的**可能是任意二进制**（图片、压缩包），不能假设它是文本 |
| `process_decode` | `String` + `read_to_string` | base64 文本**必然是 ASCII**，先按文本读；解出来的 `Vec<u8>` 再交给调用者判断是不是合法 UTF-8 |

`decode` 里的 `String::from_utf8(decode)?` 就是这道边界：**解码成功 ≠ 内容是人类可读文本**。你完全可以把一张 PNG 编码后再解码回来，`decode()` 不会报错，但 `from_utf8` 会（错误会一路冒到 `main` 打印出来）。

`let buf = buf.trim();`（[L30](file:///d:/codes/01-rcli/src/process/b64.rs#L30)）处理的是另一个现实问题：管道/编辑器/Windows 的行尾会带 `\n`、`\r\n`，而 base64 引擎不接受填充位之后的空白。

**③ `get_reader` 返回 `Box<dyn Read>` —— 全项目最值得学的 8 行**

```rust
fn get_reader(input: &str) -> Result<Box<dyn Read>> {
  let reader: Box<dyn Read> = if input == "-" {
      Box::new(std::io::stdin())        // Stdin
  } else {
      Box::new(File::open(input)?)      // File
  };
  Ok(reader)
}
```

文件里那几行注释（[L7-L11](file:///d:/codes/01-rcli/src/process/b64.rs#L7-L11)）说的就是这个问题：`if` 返回 `Stdin`，`else` 返回 `File`，**两个不同类型没有"共同类型"，函数没法返回**。解法就是"**找一个共同点**"：

- `Stdin` 和 `File` 都实现了 `Read`；
- `dyn Read` 是 **trait object**，运行时通过虚表调用，因此两个具体类型可以被塞进同一个变量；
- trait object 是 DST（动态大小），所以必须装箱 `Box`（或者用引用 `&mut dyn Read`）。

两条路对比，也是 `b64.rs` 与 `csv_convert.rs` 的写法差异来源：

| 方案 | 签名 | 特点 |
|---|---|---|
| **trait object**（本文件） | `Box<dyn Read>` | 运行时分发，可按条件切换类型；有堆分配 + 动态调用开销 |
| **泛型**（更 Rust 味） | `fn process<R: Read>(reader: R)` | 编译期单态化，更快，但**调用点必须只有一种类型**，没法在 `-` / 文件之间运行时切换 |

**④ 两种格式到底差在哪**

| | 字母表 | 填充 | 用在哪 |
|---|---|---|---|
| `STANDARD` | `+` `/` | `=` 补齐 | 邮件、MIME、一般文本 |
| `URL_SAFE_NO_PAD` | `-` `_` | 不补 | URL、JWT、文件名——因为 `+` `/` `=` 在 URL 里有特殊含义，会被转义吃掉 |

实测：`Hello, rcli base64!` → 标准 `SGVsbG8sIHJjbGkgYmFzZTY0IQ==`，url-safe 无填充 `SGVsbG8sIHJjbGkgYmFzZTY0IQ`。
`UrlSafe → url-safe` 这个 kebab-case 映射是 `ValueEnum` 自动做的，不是手写字符串。

---

## 5. 贯穿全项目的 8 条心智模型

1. **CLI 三段式**：解析（协议）→ 分发（main）→ 处理（业务）。每一段都能独立替换和测试。
2. **enum 就是穷尽的状态机**：`SubCommand` / `OutputFormat` / `Base64Format` 都不带逻辑，逻辑全在 `match`。新增一个 variant，编译器会替你列出所有需要改的地方。
3. **`?` 不只是提前返回**：它调用 `From::from` 把对方错误（`io::Error`、`DecodeError`、`Utf8Error`）转成当前返回类型的错误（`anyhow::Error`）。所以 `reader.read_to_end(&mut buf)?` 和 `STANDARD.decode(&buf)?` 能混在同一个函数里——**`anyhow` 用"类型擦除"换来了跨错误类型的简洁**。
4. **`anyhow` vs `thiserror`**：`anyhow` 给**应用**（只要把错误报给人看），`thiserror` 给**库**（调用方需要 `match` 具体错误码）。你是 CLI，所以全项目 `anyhow` 是对的；将来把 `process_*` 抽给别人当库用时再换。
5. **`-` 代表 stdin 是 Unix 惯例**（`cat -`、`curl`、`git`）。`verify_file` 特意放行 `"-"`（[src/cli/mod.rs#L31](file:///d:/codes/01-rcli/src/cli/mod.rs#L30-L36)），`get_reader` 再把 `"-"` 翻译成 `stdin`，两处配合才让 `echo hi | rcli base64 encode` 成立。
6. **字节 vs 文本的边界意识**：`gen_pass` 全程 `Vec<u8>` 最后一步 `from_utf8`；`b64` encode 收字节、decode 吐文本。**什么时候边界断裂，什么时候就报错**——这是 Rust 处理编码问题的基本姿势。
7. **clap derive 是"声明式 + 值解析"**：`#[arg(short, long, value_parser = ..., default_value = ...)]` 让"字符串 → 领域类型"的转换发生在进入结构体之前。业务函数因此**永远不用再做合法性判断**（对比 Python 里函数开头那一堆 `if args.x is None`）。
8. **测试分三层，写在离代码最近的地方**：`#[cfg(test)]` 单元测试贴在同一文件（`cli/mod.rs`、`process/b64.rs`）；`assets/` 是给人手动试的数据，`fixtures/` 是给测试用的数据；跨进程的端到端验证留给 `tests/` + `assert_cmd`（目前还没建）。

---

## 6. 命令速查（都是本仓库实测通过的）

```bash
# 1) CSV → json/yaml/toml（不给 -o 时写出 output.<format>）
cargo run -- csv -i assets/juventus.csv -o output.json -f json   # output.json/yaml/toml 都已 gitignore

# 2) 随机密码：结果走 stdout，强度提示走 stderr
cargo run -- genpass --length 20 --upper --lower --number --symbol
pass=$(./target/debug/rcli.exe genpass --length 16)   # ⚠️ 今天还拿不到干净结果，见下方说明

# 3) base64：文件 / stdin 两种输入，standard / url-safe 两种格式
cargo run -- base64 encode -i Cargo.toml
printf 'roundtrip ok' | cargo run -q -- base64 encode          # 不给 -i 时默认 "-"，即读 stdin
cargo run -- base64 decode -i fixtures/b64.txt -f url-safe     # → Hello, rcli base64!

# 4) 开发与质量
cargo test            # 当前 3 个用例全过
cargo check --all
cargo clippy --all-targets -- -D warnings
```

> ⚠️ 关于 `pass=$(...)`：**今天这条命令还拿不到干净结果**。[src/main.rs#L25](file:///d:/codes/01-rcli/src/main.rs#L25) 的调试语句 `println!("Generate passwor: {:?}", gp_opts)` 也打在 stdout 上，实测：
>
> ```
> $ ./target/debug/rcli.exe genpass --length 20 ... 2>/dev/null
> Generate passwor: GenPassOpts { length: 20, upper: true, ... }   ← 污染 stdout
> CfV0pnBFQDX3Og^f9ZFQ                                             ← 真正想要的结果
> ```
>
> 这正是 §4.2 讲的"stdout 是机器的接口"被违反的样子：**删掉那行调试打印**，`$(...)` 才会只捕获到密码。（`Password strength:` 那行本来就正确地在 stderr，`2>/dev/null` 就已经看不到了。）

---

## 7. 工程化配置为什么一个都不少

这些文件不影响功能，但**它们是"课程项目"和"可发布工具"的分界线**：

| 文件 | 工具 | 为什么需要 |
|---|---|---|
| `.pre-commit-config.yaml` | pre-commit | 提交前本地就把格式/依赖/拼写/编译/lint/测试六道关卡全过一遍，别让 CI 当垃圾桶。注意它是 `bash -c '...'` 包了 cargo 命令，且 `pass_filenames: false`（cargo 作用于整个包，不是单个文件） |
| — | `cargo fmt` | 统一风格，让 diff 只反映逻辑变化，别人读你的代码不用猜缩进 |
| — | `cargo clippy -D warnings` | 把 lint **从提示升级为错误**。仓库现在恰好有 2 个 clippy warning，说明这条 `-D warnings` 会把 CI 打红（§8） |
| — | `cargo nextest` | 逐测试进程执行，失败隔离、结果更细、速度更快；`cargo test` 的替代品 |
| `deny.toml` | `cargo deny` | 检查依赖的**许可证**与**已知漏洞(RUSTSEC)**。`Cargo.toml` 里 `serde_yaml` 那条注释就是它的成果：官方 `serde_yaml` 已停止维护（RUSTSEC-2024-0320），故换成社区分支 `serde_yaml_ng` 并用 `package = ` 重命名，**代码里的 `serde_yaml::` 路径一行都不用改** |
| `_typos.toml` | `typos` | 拼写检查，`extend-exclude` 排掉自动生成的 CHANGELOG |
| `cliff.toml` + `CHANGELOG.md` | `git-cliff` | 从 **conventional commits**（`feat:`/`fix:`/`chore:`）自动生成长变更日志；这就是为什么提交信息必须按格式写 |
| `.github/workflows/build.yml` | GitHub Actions | `fmt --check` → `check` → `clippy -D warnings` → `nextest` → 打 tag 时用 cliff 生成 release body |

`git tag v1-8-genpass` 这类标签 + `fetch-depth: 0`（拉全部历史，cliff 需要）是配套关系：**版本 = tag，变更日志 = tag 之间的 commit 分类**。

---

## 8. 当前代码的问题清单（按优先级，均为实测确认）

### P0 — 真实 panic

**`genpass` 长度太短会整数下溢崩溃。** [src/process/gen_pass.rs#L38](file:///d:/codes/01-rcli/src/process/gen_pass.rs#L38)：

```rust
for _ in 0..(length - password.len() as u8) {
```

第 1 步已经按"启用的字符类数量"至少塞了 N 个字符，如果 `length < N` 就是 `3 - 4`。实测：

```
$ ./target/debug/rcli.exe genpass --length 3 --upper --lower --number --symbol
thread 'main' panicked at src\process\gen_pass.rs:38:17:
attempt to subtract with overflow        (exit code 101)
```

修法任选：① 循环长度用 `length.saturating_sub(password.len() as u8)`；② 在 `process_genpass` 开头 `ensure!(length as usize >= password.len(), "password too short")`（**推荐，明确告诉用户**）；③ 在 clap 层加校验。

### P1 — 会让 CI 变红

- `cargo fmt -- --check` 目前**不过**：18 处 diff，覆盖 `src/` 下全部 7 个 `.rs`（项目是手写 2/4 空格混排，rustfmt 是 4 空格）。一次性 `cargo fmt` 归一即可，但会产生一个大 diff，建议**单独提交**。
- `cargo clippy` 2 个 `needless_borrows_for_generic_args`：[src/process/b64.rs#L33-L34](file:///d:/codes/01-rcli/src/process/b64.rs#L33-L34) 的 `STANDARD.decode(&buf)`，改成 `decode(buf)`。CI 里是 `-D warnings`，所以这两条会直接失败。

### P1 — 功能上的"空头承诺"

`CsvOpts` 声明了 `-d/--delimiter` 和 `--header`（[src/cli/csv.rs#L16-L20](file:///d:/codes/01-rcli/src/cli/csv.rs#L16-L20)），但 `main.rs` 只往下传了 `input/output/format` —— **这两个参数用户能传、传了却毫无作用**。这类"接口有、实现无"的口子最危险。要么接线（`Reader::from_path` 后按 `delimiter` 构造 `ReaderBuilder`、`header` 决定首行是否当 key），要么先从 CLI 删掉。

### P2 — 债务与噪音

- `struct Player` 是 never-constructed 死代码（每次构建都刷 warning）。删掉，或留着当下一课（"用 `Deserialize` 直接读成强类型 + `serde` 属性"）的锚点，那就 `#[allow(dead_code)]` + 一行注释说明意图。
- [src/main.rs#L25](file:///d:/codes/01-rcli/src/main.rs#L25) 的 `println!("Generate passwor: {:?}", gp_opts)` 是调试残留，且**直接破坏 `pass=$(rcli genpass)` 这类管道用法**（它打在 stdout，结果被污染，见 §6 实测），`passwor` 还少了个 `d`。这类调试打印要么删掉，要么用 `eprintln!`。
- [src/process/csv_convert.rs#L23](file:///d:/codes/01-rcli/src/process/csv_convert.rs#L23) 的 `Vec::with_capacity(128)` 是魔法数：它只是预分配提示，行数超过 128 也照样能跑，但读代码的人会以为是上限。
- `verify_file` 的错误信息是固定的 `"File does not exist"`，不带文件名，用户不知道是哪个参数错了（建议：`format!("{filename} does not exist")`，注意返回类型是 `&'static str` 时没法带动态内容，需要换成 `String`/`anyhow`）。
- 模块路径有三种写法并存：`cli/` 内用 `super::`、`process/` 用 `crate::cli::OutputFormat`、`main.rs` 用 `rcli::Base64Format`。**每种单独看都合理**（见 §2.1 第 3 点），但没有成文约定，读代码时会误以为有什么特殊含义。建议定一条：**crate 内部一律 `crate::`，只有 bin 和外部用户走 `lib.rs` 门面**（`cli/mod.rs` 那行注释就是这条约定的雏形）。
- 数据目录三分天下：`assets/`（手工试）、`fixtures/`（测试用，本次为 `test_process_decode` 新建）、`texitures/`（**空目录 + 拼写错误**）。删掉 `texitures/`，或把它正名为 `textures/` 并统一成一处。
- `lib.rs` 的 `pub use process::*;` 是 glob 再导出：省事，但**公开 API 面看不见**，将来加同名函数会静默冲突。
- **测试断言太弱，内容被改了都不知道。** 现场事故：本文写作过程中，`fixtures/b64.txt` 被一次 shell 重定向意外覆盖成了"Cargo.toml 的 base64"，`cargo test` **依然 3 passed** —— 因为 `test_process_decode` 只写了 `process_decode(input, format).unwrap()`（断言"没报错"），`test_process_encode` 只写了 `is_ok()`（[src/process/b64.rs#L55-L67](file:///d:/codes/01-rcli/src/process/b64.rs#L55-L67)）。只要 `process_*` 还在函数内部 `println!` 而不返回结果，测试就**永远只能测"有没有崩"，测不出"对不对"**。这正是练习 2 的意义。
- `process_encode` 一次性 `read_to_end`，大文件全进内存；`base64` 其实支持流式（`base64::write::EncoderWriter` / `base64::read::DecoderReader`，注意它们是 0.23 的 `read`/`write` 子模块，`Engine` trait 上并没有 `encode_reader` 这种方法）。

### P3 — 模板残留

- `CHANGELOG.md` 里的 commit 链接指向 `github.com/tyrchen/qdrant-lib`（工程化模板带过来的），`cliff.toml` 虽然改过仓库 URL（见 `chore: currect the project url in cliff config`），历史 CHANGELOG 还没重新生成。
- `deny.toml` 基本还是上游模板的注释状态（配置项全是 `#` 注释），等于"跑了但没真正约束"。
- `Cargo.toml` 的 `license = "MIT OR Apache-2.0 OR Unicode-DFS-2016 OR MPL-2.0 OR BSD-2-Clause OR BSD-3-Clause OR ISC OR CC0-1.0"` 太长，`OR` 列表意味着"用户可以任选一个许可证"，一般就写 `"MIT OR Apache-2.0"`。
- `build.yml` 的 `platform: ["ubuntu:22.04"]` 是容器镜像（作者环境用 docker runner），Windows 本地开发时这些 hook 需要 WSL/Git Bash 才跑得到。

### 建议的下一步练习（按学习收益排序）

1. **把 P0 修掉并补一个回归测试**：`process_genpass` 目前完全没测试，`--length 1` 全开的情况应该返回 `Err` 而不是 panic。
2. **给 `process_*` 换成"接受 `impl Read` / 返回 `String`"的签名**，把 `println!` 挪回 `main.rs`，业务函数立刻变得可断言（现在测试只能靠 `is_ok()`，测不到输出内容）。
3. **接上 `delimiter` / `header`**，练 `csv::ReaderBuilder`。
4. **加 `tests/` 集成测试**（`assert_cmd` + `predicates`），第一次真正验证 stdout，而不是"没报错就算过"。
5. **把 `--verbose` / `RUST_LOG` 加进来**，体会 stderr 分级的意义。
6. 想发出去的话：`cargo publish` 前的 `cargo deny check`、跨平台 matrix（`windows-latest` + `macos` + `ubuntu`）、`cargo-dist` 打二进制包。

---

## 9. 一句话回答"为什么要写这些"

`csv` 教你 **serde 的数据模型**，`genpass` 教你 **字节与随机以及 Unix 的输出规矩**，`base64` 教你 **trait object 和二进制↔文本的边界**，`clap` 层教你 **把字符串升级成类型**，`lib.rs`/`main.rs` 的切分教你 **可测试的代码长什么样**，那一堆配置文件教你 **一个能给别人用的工具除了功能还需要什么**。

数据流走通只是第一课；上面 §8 里那些"能跑但有味道"的地方，恰好才是 Rust 真正在约束你的东西。
