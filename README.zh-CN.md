# glowclock

[English](./README.md)

终端里的渐变大字时钟(tty-clock 风格),带 **crontab 风格的定时提醒** 和一只 **胖猫**。到点时胖猫会弹出来提醒你(比如每小时喝水),并播放提示音(可用 `--sound` 配置)。

用 Rust + [ratatui](https://ratatui.rs) + crossterm 写成,只依赖这两个库,本地时间通过系统 `date` 读取时区偏移,不引入 `chrono`。

```
 ┌ GLOWCLOCK  theme: aurora ┐
        Thu 2026-09-10
    ██████   ██████   ...        ← 逐行青→蓝渐变的大字
    q quit  space theme  ...
```

---

## 安装

```bash
# Homebrew(macOS / Linux)
brew install tomtdhzz/tap/glowclock

# 或一行安装脚本(预编译二进制)
curl -LsSf https://github.com/tomtdhzz/glowclock/releases/latest/download/glowclock-installer.sh | sh
```

每个 [release](https://github.com/tomtdhzz/glowclock/releases) 都附带 macOS(arm64/x86_64)与 Linux(arm64/x86_64)的预编译二进制。

---

## 构建与运行

```bash
cd glowclock
cargo build --release
./target/release/glowclock            # 启动交互式时钟
```

首次运行若当前目录有 `glowclock-reminders.txt`,会自动加载其中的提醒;否则用内置默认提醒(每小时喝水 + 每 45 分远眺)。

---

## 交互按键

| 按键          | 作用                     |
| ------------- | ------------------------ |
| `q` / `Esc`   | 退出                     |
| `a`           | 添加提醒(直接输入)     |
| `space` / `c` | 切换主题(配色)         |
| `f`           | 切换 12/24 小时制        |
| 任意键        | 关闭当前的胖猫提醒弹窗   |

> 有弹窗时,除 `q`(始终退出)外的任意键都会先关闭弹窗。

---

## 时钟显示

- **大字**由内置 5×7 位图字体放大而成,颜色是**逐行垂直渐变**(顶部亮、底部深),这就是"发光"的观感。
- **冒号常驻不闪**,只有数字跳动。冒号是纤细的 1px 双点,不会和数字挤在一起。
- **自适应缩放**:根据窗口大小自动选择合适的字号,并保证数字之间始终有间距(不会糊成一团)。
- **极窄窗口兜底**:当窗口窄到连最小字号的 `时:分:秒` 都放不下时,会**自动去掉冒号**(数字仍带间距,依然可读)。
- 时钟上方显示日期与星期,如 `Thu 2026-09-10`。

### 主题

内置 4 套:`aurora`(默认,青→蓝)、`sunset`(橙→粉)、`matrix`(绿)、`ice`(冷白→蓝灰)。

```bash
./target/release/glowclock --theme sunset
./target/release/glowclock --theme 2       # 也可用序号
./target/release/glowclock --gallery       # 一次预览全部主题(输出到 stdout)
```

运行时按 `space` 循环切换。

---

## 定时提醒(crontab 风格)

### 提醒文件格式

一行一条提醒,`#` 开头或空行会被忽略。每行由**调度表达式** + **消息文本**组成。

**1) 经典 5 段 cron**

```
分  时  日  月  周   消息
```

| 字段 | 取值范围 | 说明                          |
| ---- | -------- | ----------------------------- |
| 分   | 0–59     |                               |
| 时   | 0–23     |                               |
| 日   | 1–31     | 一月中的第几天                |
| 月   | 1–12     |                               |
| 周   | 0–7      | 0 和 7 都表示周日,1=周一…6=周六 |

每个字段支持:

- `*` —— 任意值
- `N` —— 具体值(如 `30`)
- `A-B` —— 范围(如 `9-18`)
- `*/S` —— 步长(如 `*/30` 每 30)
- `A-B/S` —— 范围内步长(如 `0-30/10`)
- `a,b,c` —— 列表(如 `0,15,30,45`)
- 「周」字段还支持名字 `sun`–`sat`,「月」字段支持 `jan`–`dec`(如 `5 4 * * sun`)

> 与标准 cron 一致:当"日"和"周"都不是 `*` 时,两者**满足其一**即触发;只要有一个是 `*`,则按另一个匹配。

**2) 便捷关键字**

```
@minutely   消息      # 等价 * * * * *
@hourly     消息      # 等价 0 * * * *
@daily      消息      # 等价 0 0 * * *(每天 0 点)
```

**3) 固定间隔**

```
@every <时长>   消息
```

时长写法:`90s`、`30m`、`1h`、`1h30m`、`2d`,也可直接写数字(按秒)。从程序启动那一刻开始计时,之后每隔该时长触发一次。

### 示例

```crontab
# 分 时 日 月 周   消息
0 * * * *          该喝水啦！起来动一动 (=^.^=)
*/30 9-18 * * 1-5  工作日每半小时:活动颈椎、远眺一下
0 12 * * *         午饭时间到,喂喂胖猫 🐟
0 18 * * 1-5       下班前:回顾今天做了什么
@every 45m         眨眨眼,别盯屏幕太久
```

仓库里的 `glowclock-reminders.txt` 就是这样一份可直接编辑的示例。

### 加载顺序

程序按以下顺序寻找提醒来源,取第一个可用的:

1. `--reminders <路径>` 指定的文件
2. 当前工作目录下的 `glowclock-reminders.txt`
3. `~/.config/glowclock/reminders.txt`
4. 内置默认提醒

```bash
./target/release/glowclock --reminders ~/my-reminders.txt
./target/release/glowclock --list-reminders   # 打印当前加载了哪些提醒(含来源、解析错误)
```

解析出错的行会在启动时打到 stderr,并被跳过,不影响其它行。

### 用命令行管理提醒

不想手动改文件的话,可以直接用命令行增/查/删提醒,它们会写入提醒文件(见下方"写入哪个文件")。

也可以**不退出时钟直接加**:按 `a`,输入提醒(和文件里写法一样,界面里不用给 `*` 加引号),回车即可。输入时会**实时预览**这条 cron 的含义和下次触发时间(如 `5 4 * * sun` → `每周日 04:05 · 下次 …`),并附符号说明。保存后**立即生效**,胖猫会弹出 `已添加提醒:…` 确认。

```bash
# 添加:  glowclock add <调度> <消息...>
glowclock add @hourly 喝水                # 便捷关键字,无需引号
glowclock add @every 45m 远眺             # 间隔,无需引号
glowclock add "0 9 * * 1-5" 开晨会        # 原始 cron:一定要加引号,否则 shell 会把 * 展开成文件名

glowclock list                           # 带序号列出提醒
glowclock rm 2                            # 删除第 2 条
```

> 原始 cron 表达式(带 `*`)务必用引号包住 `"0 9 * * 1-5"`,否则 shell 会把 `*` 展开成当前目录的文件名。`@hourly`/`@daily`/`@every` 不含 `*`,不用引号。

**写入哪个文件**:给了 `--reminders <路径>` 就写它;否则写第一个已存在的默认文件;都不存在则写 `~/.config/glowclock/reminders.txt`(自动创建)。把 `--reminders <路径>` 放在子命令**前面**可指定目标文件:

```bash
glowclock --reminders ~/my-reminders.txt add @hourly 喝水
```

### 提醒如何出现 / 如何关闭

- **出现**:到点时,胖猫会**从屏幕右侧滑出**,旁边带一个显示消息的对话气泡(像通知,而不是居中弹窗),同时播放提示音。
- **提示音**:用 `--sound` 配置 —— `off`(静音)、`bell`(终端铃,默认)、macOS 系统声音名(如 `Glass`、`Ping`,见 `--list-sounds`),或一个音频文件路径(用 `afplay` 播放)。
- **触发一次**:cron 提醒在匹配的那一分钟内只触发一次;`@every` 每个周期触发一次。
- **关闭**:按任意键立即关闭;或 **60 秒后自动消失**。弹窗显示期间不会被下一条提醒打断。
- **退出程序**:`q` 或 `Esc`。

---

## 定义胖猫

### 选内置的猫

内置:`chonk`(默认)、`kitten`、`loaf`、`sleepy`、`peek`。

```bash
./target/release/glowclock --cat kitten
./target/release/glowclock --list-cats            # 预览默认猫
./target/release/glowclock --list-cats --cat loaf # 预览指定猫
```

### 用自己的猫

用 `--cat-file` 指定一个纯文本文件,**每行就是猫的一行**(照原样显示,可用任意 Unicode/ASCII 字符):

```bash
./target/release/glowclock --cat-file ~/mycat.txt
```

`mycat.txt` 示例:

```
 /\_/\
( o.o )
 > ^ <
```

> 提示:弹窗宽度会按猫和消息的显示宽度自动撑开;中日韩全角字符按 2 列计算。想要边框对齐美观,尽量让每行宽度接近。

`--cat-file` 优先于 `--cat`;都不指定则用默认的 `chonk`。

---

## 命令行参考

```
glowclock [选项]
glowclock <子命令> ...

子命令(管理提醒文件):
  add <调度> <消息...>   添加一条提醒(原始 cron 记得加引号:"0 9 * * 1-5")
  list                   带序号列出提醒
  rm <序号>              删除指定序号的提醒

模式(默认进入交互式时钟):
  --snapshot          把一帧真彩渐变时钟以 ANSI 输出到 stdout 后退出
  --gallery           每个主题各输出一帧后退出
  --plain             输出一帧纯方块(无颜色),便于在日志/管道里查看
  --list-reminders    打印已加载的提醒(来源、条目、解析错误)后退出
  --list-cats         打印将要使用的猫(配合 --cat/--cat-file)后退出
  --list-sounds       打印可用的提醒提示音后退出

选项:
  --theme <name|N>    主题:aurora sunset matrix ice(或序号 0..3)
  --time HH:MM:SS     显示固定时间(用于截图/调试,不再走秒)
  --reminders <path>  从指定的 crontab 风格文件加载提醒
  --cat <name>        选内置猫:chonk kitten loaf sleepy peek
  --cat-file <path>   从文件加载自定义猫(每行一行)
  --sound <spec>      提醒提示音:off | bell | <macOS 声音名> | <音频路径>
  --12 | --24         12/24 小时制(默认 24)
  -h, --help          帮助
```

---

## 实现说明

- **时间/日期**:`src/clock.rs`。时区偏移一次性从 `date +%z` 读取;用 Howard Hinnant 的历法算法从 UNIX 时间戳换算出年/月/日/星期,不依赖 `chrono`。
- **字体与渐变**:`src/font.rs`(5×7 位图) + `src/render.rs`(缩放 + 逐行 RGB 渐变 + 自适应字号)。
- **提醒**:`src/reminder.rs`。自实现的 cron 字段解析(位掩码匹配) + `@every` 间隔调度 + 触发去重。
- **胖猫**:`src/mascot.rs`。内置若干 ASCII 猫,支持按名字选择或从文件加载。
- **界面**:`src/main.rs`。ratatui 布局 + `Clear` 叠加弹窗。

### 测试

```bash
cargo test --release
```

覆盖:历法日期/星期换算、cron 解析与匹配(步长/范围/列表/周日 0-7/日或周语义)、时长解析、间隔与整点各触发一次、字号自适应与"永不 0 间距"、渐变取色、猫的解析等。
