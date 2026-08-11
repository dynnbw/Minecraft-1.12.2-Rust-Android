# Minecraft Java Edition 1.12.2 — Rust Semantic Port (Android)

> 使用 Rust 对 Minecraft Java Edition 1.12.2 客户端进行语义级移植,当前重心为 **Android 平台(协议 340,Vulkan 渲染,物理键鼠操作)**。

> 使用 Rust 对 Minecraft Java Edition 1.12.2 客户端进行语义级移植，并提供 Vulkan 与 OpenGL 双渲染后端。

当前公开基线：**0.127.0**<br>
当前重点平台：**Windows 10/11 x64**<br>
协议目标：**Minecraft Java Edition 1.12.2 / Protocol 340**<br>
Android 基线:**0.112.1**(同步上游 v0.112.1)<br>
验证设备:**Xiaomi 12 Pro**(Adreno 730,Android 12+,Vulkan 1.1)

---

## 注意!!!

项目需要导入 1.12.2 资源。手动导入十分复杂,推荐使用自动导入脚本,但自动导入脚本不一定能成功运行,这主要取决于你的环境与目标资源文件。可以使用本机的 1.12.2 资源,但推荐使用官方重写参照的 MCP-1.12.2-main.zip,这个自动导入资源脚本成功运行率更高:https://1850640083.share.123pan.cn/123pan/RvAxvd-NIOgA

## 项目简介

本项目的目标不是制作一个"外观类似 Minecraft"的独立仿制游戏,而是在 Rust 中尽可能忠实地重建 Minecraft Java Edition 1.12.2 客户端的行为、状态、调用流程、网络协议、GUI、模型、动画和操作体验。原版 Minecraft 1.12.2 与对应 MCP 代码结构是行为基准。

**Android 版定位**:远程多人客户端,Vulkan 渲染,物理键鼠为主,触屏通过桥接层映射为键鼠操作(点击/滑动/长按)。项目本身不含单人集成服务器。

项目当前已经同时具备远程多人客户端与正在迁移中的单人 IntegratedServer 路径。v0.127.0 已实现 Flat 世界的真实 LocalChannel/IntegratedServer 进入链、服务器权威区块修改与 Anvil/playerdata 持久化，并已把 Default/Default 1.1/Large Biomes/Amplified/Customized 的主世界生成推进到 MCP 派生的 GenLayer、BiomeProvider、Noise、Biome surface、洞穴和峡谷主干。这仍不等于整个 Minecraft 1.12.2 已无差异完成：结构生成、population/decorator、Nether/End、完整 Entity/TileEntity 生命周期以及部分复杂交互仍在继续迁移。

## 功能状态(Android)

### 已验证

- APK 装机即玩:资源打包进 APK,首次启动自动解压到内部存储
- 主菜单完整渲染(全屏 2400x1080,约 29 fps)
- 物理键鼠:光标悬停、左右键点击、滚轮、键盘输入
- 远程服务器连接(协议 340,legacy 离线会话)
- Vulkan 渲染全链路:Adreno 730 设备、共享区块显存池、间接多绘制
- 游戏内第一人称交互(移动/视角/背包):物理键鼠与触摸桥接均可用
- 触摸桥接:点击热键栏切换物品、长按(3s)丢弃;世界点击放置(右键)、长按摧毁(左键)、滑动转视角
- 音频:rodio → cpal → AAudio(oboe),与桌面共用同一播放后端(含 3D 衰减),声音资源完整打包
- 切后台保活:拉下通知栏 / 按 Home 后回到前台,直接恢复世界画面与服务器会话(不重建主菜单、不崩溃)

### 已知限制

- **触摸桥接而非触摸 UI**:触屏输入映射为键鼠操作(无虚拟摇杆/触摸按钮),聊天等需要文本的界面仍需物理键盘
- **视角转动是相对增量**:进入世界后通过窗口指针捕获(pointer capture)锁定物理鼠标,系统直接上报相对位移;菜单中仍为绝对光标
- **系统栏区域未覆盖**:Android 12+ 上系统栏(横屏时位于侧边)仍占约 106px,全屏沉浸待完善
- **无 OptiFine 光影**:光影为 OpenGL 后端专属,Android 仅 Vulkan
- **无软键盘 IME**:聊天输入需要物理键盘
- **单机不可用**:单人集成服务器不在项目基线

### 明确不做(当前范围)

- 触摸 UI(虚拟摇杆等)
- OpenGL 后端
- Microsoft 登录 UI(桌面版已有内置账号管理器,Android 侧未接入)

## v0.127.0 当前进展

- Flat 单人世界通过真实 `IntegratedServer -> LocalChannel -> Login/Play -> WorldClient` 链进入，不使用客户端静态地形替代服务器。
- 方块修改开始由 `PlayerInteractionManager / WorldServer / Chunk` 权威处理，并进入 Anvil 异步保存；玩家位置、背包、当前槽位等写入 `playerdata/<UUID>.dat`。
- Default、Default 1.1、Large Biomes、Amplified、Customized 已接入真实 `IntCache / GenLayer / BiomeProvider / NoiseGenerator / ChunkGeneratorOverworld` 基础地形，并包含 biome surface、洞穴和峡谷。
- Vulkan/OpenGL 继续共享 MCP 派生场景状态；OpenGL 已包含 resident-span 局部 `BufferSubData` 更新等性能路径。
- 尚未完成的主项包括 Overworld structures 与 population/decorator、Nether/End generator、完整 TileEntity/复杂多方块放置、完整服务端实体生命周期与更多单人服务器行为。

## 主要特色


### Minecraft 1.12.2 语义结构

源码目录尽量镜像 MCP 包路径，包括：

- `net.minecraft.client`
- `net.minecraft.entity`
- `net.minecraft.block`
- `net.minecraft.item`
- `net.minecraft.network`
- `net.minecraft.world`
- `net.optifine`

项目包含协议 340 的登录、加密和多人游戏数据路径，以及 GUI、HUD、物品栏、容器、资源包、声音、玩家皮肤/披风、方块状态、实体渲染、粒子和维度相关实现。当前版本还包含内置账号管理器、Microsoft 浏览器 OAuth、Token/Offline 会话切换和远程玩家名称标签。各系统的完成程度并不完全相同，公开发布时不应将本项目描述为原版客户端的无差异替代品。

### Vulkan 渲染后端

Vulkan 路径使用 Vulkan 1.1，主要技术包括：

- 原版 `RenderChunk`、`CompiledChunk` 与 `VisGraph` 可见性结构；
- `SOLID`、`CUTOUT_MIPPED`、`CUTOUT`、`TRANSLUCENT` 四个方块渲染层；
- 共享区块顶点/索引显存池；
- 设备本地常驻区块网格；
- 多绘制间接命令；
- 有界区块编译和上传队列；
- 原版透明四边形中心距离排序与运行时重新排序；
- 动态实体、方块实体和静态悬挂实体独立 GPU 流；
- Vulkan 原生 GUI、全景主菜单和异步纹理上传；
- 帧槽 Fence 驱动的资源延迟回收。

### OpenGL 渲染后端

OpenGL 路径创建 OpenGL 3.3 Compatibility Profile，主要技术包括：

- 与 Vulkan 共用的 MCP 场景构建结果；
- `RenderRegion` 驻留和 `MultiDrawElements`；
- 精确透明索引区间更新；
- 原版实体与方块实体程序边界；
- OptiFine 1.12.2 风格的 G-buffer、composite、final 与 shadow 程序路径；
- Shader Options、维度目录、include 展开和光影包配置。

**OptiFine 光影仅在 OpenGL 后端启用。** Vulkan 后端不会直接运行传统 OptiFine GLSL 光影包。

### 内置账号管理器

主菜单中的 `Accounts` 页面提供本地账号列表和会话切换，当前支持：

- Microsoft 浏览器 OAuth 登录；
- 已保存 Microsoft 账号的访问令牌登录和刷新令牌续期；
- Minecraft Access Token 登录；
- `M.C` 刷新令牌登录；
- Offline 用户名会话；
- 账号排序、删除、双击登录、头像显示和当前账号高亮；
- 使用当前 Minecraft Access Token 上传 Classic 或 Slim 皮肤。

认证成功后会替换客户端真实 `Session`，并继续使用 1.12.2 的 `NetHandlerLoginClient → joinServer` 认证链，不是只修改界面用户名。

账号数据保存在：

```text
config/account.json
```

为保持与参考账号管理器兼容，该文件包含明文刷新令牌和 Minecraft Access Token。仓库的 `.gitignore` 已忽略整个 `config/`，但提交前仍必须检查 Git 变更列表，确保没有通过强制添加、旧提交或其他路径泄露账号凭据。

### 远程玩家名称标签

玩家名称标签按 Minecraft 1.12.2 的 `RenderLivingBase`、`RenderPlayer`、`ScorePlayerTeam` 与 `Scoreboard` 行为实现，包括：

- 普通玩家 64 格、潜行玩家 32 格显示距离；
- 队伍前缀、后缀、颜色与四种名称可见规则；
- 友军隐身可见和旁观者相关判断；
- 普通名牌的穿墙暗色层与深度测试亮色层；
- 潜行名牌的遮挡和深度写入规则；
- 10 格内显示记分板显示槽 2 的分数与目标名称；
- 第三人称不显示本地玩家自己的名称。

### 资源系统

项目不会在仓库中捆绑客户端运行所需的完整 Mojang 资源集合。源码中仅保留构建或界面所需的少量内嵌元数据与默认图标；完整纹理、声音、字体、模型和语言资源仍由维护者或用户从合法本地 Minecraft 安装和 MCP 资源中导入。

资源导入完成后位于：

```text
runtime/assets/
└─ minecraft/
   ├─ blockstates/
   ├─ lang/
   ├─ models/
   ├─ sounds/
   ├─ textures/
   ├─ optifine/
   └─ mcpatcher/
```



## 安卓构建与打包

### 环境要求

| 工具 | 说明 |
|---|---|
| Rust stable + aarch64-linux-android 目标 | `rustup target add aarch64-linux-android` |
| cargo-ndk | `cargo install cargo-ndk` |
| Android NDK | r25+ 均可(验证使用 r29) |
| Android SDK build-tools / platforms | aapt2、zipalign、apksigner 需要 |
| Python 3.9+ | 打包脚本 |
| Java 11+ | apksigner 运行 |
| 已导入的 `runtime/assets` | 见[资源导入](#资源导入) |

### 构建步骤

**1. 预编译 SPIR-V 着色器(Windows 上执行一次,产物入库)**

```bash
cargo run --manifest-path tools/spv-precompiler/Cargo.toml --release
```

生成 `src/vulkan/shaders/spv/*.spv`(7 个文件)。Android 构建跳过 shaderc 交叉编译,直接嵌入这些预编译产物。

**2. 交叉编译 cdylib**

```bash
cargo ndk -t arm64-v8a --platform 31 -o target/android build --release --lib
```

产出 `target/android/arm64-v8a/libminecraft_1_12_2_rust_vulkan.so`(注意用 `--lib`,`--bin` 不会产出 APK 所需的 cdylib)。

**3. 打包 APK**

```bash
python tools/build_apk.py
```

产出 `dist/Minecraft112Rust.apk`(约 123 MiB,含全部声音资源与原生库)。脚本自动:
- 写入 `mcassets.list` 解压清单(跳过点文件)
- `aapt2 compile` 编译 `res/` 资源(启动图标)并链接
- 附带 `libc++_shared.so`(oboe 音频库的 C++ 运行时,NDK r27+ 位于 toolchain sysroot)
- aapt2 link + zipalign + apksigner 签名(复用系统 debug keystore)

**4. 安装**

```bash
adb install -r dist/Minecraft112Rust.apk
```

### 会话配置(launcher.json)

Android 无命令行参数,会话信息读取 `<内部存储>/launcher.json`:

```json
{"username":"AndroidPlayer","player_id":"00000000-0000-0000-0000-000000000000","access_token":"","user_type":"legacy"}
```

写入方式(debuggable 安装):

```bash
adb shell "run-as net.mc112rust.client sh -c 'cat > launcher.json'" <<'EOF'
{"username":"AndroidPlayer","player_id":"00000000-0000-0000-0000-000000000000","access_token":"","user_type":"legacy"}
EOF
adb shell am force-stop net.mc112rust.client
adb shell am start -n net.mc112rust.client/android.app.NativeActivity
```

`user_type` 默认 `legacy`;连接需要正版会话的服务器时,由外部启动器或代理提供会话信息。

## 技术架构(Android)

- **入口**:cdylib 导出 `android_main`(NativeActivity),与桌面 `main()` 共享同一个 `Main::main` 游戏入口
- **窗口/事件**:winit 0.30 `ApplicationHandler` 模型(resumed/suspended/window_event/device_event)
- **winit 鼠标补丁**(`tools/winit-patched`):上游 winit 0.30.13 未实现 Android 鼠标事件。补丁将 `SOURCE_MOUSE` 事件映射为 `CursorMoved`/`MouseInput`/`MouseWheel`,并合成 `DeviceEvent::MouseMotion` 相对增量;同时让 `set_cursor_grab` 接受请求(Android 无原生光标锁定,游戏以此进入第一人称模式)
- **指针捕获(相对鼠标)**:游戏进入第一人称时经 JNI 请求窗口指针捕获(`View.requestPointerCapture`,UI 线程执行)。捕获后系统隐藏并锁定光标,鼠标事件切换为 `SOURCE_MOUSE_RELATIVE`,X/Y 直接携带位移增量,视角转动不再受屏幕边缘限制;菜单打开/失焦时自动释放
- **Vulkan 适配**:
  - swapchain 尺寸读取 ANativeWindow 真实值(而非 winit 报告值)
  - 容忍 `VK_SUBOPTIMAL_KHR`(Adreno 在非 IDENTITY transform 下每帧报告;重建会使帧循环降到 ~1 fps)
  - `pre_transform` 保持 IDENTITY 保证横屏内容方向
- **后台保活**:切后台时保留渲染器、winit 窗口与游戏会话,仅暂停绘制(表面随原生窗口销毁);回前台时重新获取 Vulkan surface + swapchain 直接恢复世界画面,失败则降级重建主菜单
- **音频**:rodio → cpal → oboe(AAudio),与桌面共用同一播放后端(含 3D 衰减);ndk-context 由 android-activity 预初始化;APK 附带 `libc++_shared.so`
- **触摸桥接**:触摸事件在游戏侧映射为键鼠语义(热键栏命中/长按计时),长按判定在 `about_to_wait`、触摸 Moved、触摸 Ended 三处触发,并保证 3 秒整点唤醒
- **资源引导**:`AssetBootstrap` 通过 AssetManager 按 `mcassets.list` 清单解压到内部存储,标记文件避免重复解压(内容变更时升级标记版本触发重解压)
- **Vulkan 加载**:ash 动态加载 `libvulkan.so`(非链接方式)
- **日志**:android_logger 输出到 logcat(tag `mc112`),`adb logcat -s mc112:*`

## 资源导入

Android 与桌面共用同一资源流程:导入结果位于 `runtime/assets`,打包时整体进入 APK。

推荐使用一键脚本(Windows):

```bat
python tools\one_click_import_assets.py --project-root . --minecraft-dir "%APPDATA%\.minecraft" --mcp ".\MCP-1.12.2-main.zip"
```

导入成功后生成 `runtime/assets/` 与 `runtime/asset-import-report.json`。手动导入步骤与校验见仓库历史 README 或原项目仓库。

## 目录结构(Android 相关)

```text
.
├─ AndroidManifest.xml            # NativeActivity 清单(横屏)
├─ res/mipmap/                    # 应用图标(打包时编译进资源表)
├─ tools/
│  ├─ build_apk.py                # APK 打包脚本(aapt2/zipalign/apksigner,含 libc++_shared.so)
│  ├─ spv-precompiler/            # SPIR-V 预编译工具(独立 crate)
│  └─ winit-patched/              # winit 0.30.13 副本 + Android 鼠标补丁
├─ src/
│  ├─ lib.rs                      # android_main 入口(cdylib 导出)
│  └─ launcher/
│     ├─ android.rs               # AndroidApp 全局、gameDir、沉浸、指针捕获 JNI
│     ├─ AssetBootstrap.rs        # 首启资源解压
│     └─ AndroidLaunchConfig.rs   # launcher.json 解析
└─ dist/                          # 打包产物(APK,gitignored)
```

## 桌面版(Windows)

桌面基线保持双后端渲染:Vulkan 1.1 与 OpenGL 3.3 Compatibility Profile(OptiFine 光影仅 OpenGL)。v0.112.1 起包含内置账号管理器:主菜单 `Accounts` 页面支持 Microsoft 浏览器 OAuth 登录、Token/Offline 会话切换、已保存账号管理、皮肤上传,账号数据保存在 `config/account.json`(明文令牌,勿提交)。另含远程玩家名称标签(队伍前缀/颜色、可见规则、潜行名牌)。构建:

```bat
cargo build --release --bin mc112-client
target\release\mc112-client.exe run --assets runtime/assets
```

常用参数:`--assets`、`--width`、`--height`、`--fullscreen`、`--username`、`--uuid`、`--accessToken`、`--userType`。渲染后端在视频设置中切换(`rustRenderBackend:vulkan` / `:opengl`,需重启)。

材质包放入 `resourcepacks/`,光影包放入 `shaderpacks/`(仅 OpenGL 后端)。

## 当前边界

- 项目不是 Mojang 官方客户端，也不是 Minecraft 1.12.2 的完成版替代品
- 主要运行路径是远程多人客户端;Android 侧单机集成服务器不在当前基线
- Flat IntegratedServer 已能真实创建/进入；方块和玩家状态的服务端权威保存链已开始闭合，但复杂 TileEntity、多方块、红石依赖交互仍需继续补齐
- Default / Default 1.1 / Large Biomes / Amplified / Customized 已有真实 MCP 派生基础地形、biome surface、洞穴和峡谷；村庄、矿井、要塞、神殿、海底神殿、林地府邸以及湖泊、地牢、矿物、树木、花草等 population/decorator 仍在迁移
- Nether、End 与 Debug generator 尚未达到完整 1.12.2 行为
- 部分少见实体、TileEntity、交互或视觉边缘情况仍可能与原版存在差异
- 任意第三方 OptiFine 光影包的普遍兼容性未作保证
- 内置 Microsoft 登录依赖 Microsoft/Xbox/Minecraft 在线认证服务，服务端策略、二次验证或账号状态可能导致登录失败
- `config/account.json` 保存明文令牌，只能保留在可信本地环境中
- 仓库不捆绑原版资产，因此首次运行前必须从合法本地来源导入资源

发现差异时，应提供：

1. Minecraft 1.12.2 原版表现；
2. 本项目表现；
3. 可复现步骤；
4. Vulkan 或 OpenGL 后端；
5. 使用的世界类型、资源包/光影包；
6. 完整日志；
7. 对应 MCP 类或方法（如可确定）。

## 开发与贡献约束

1. 先定位原版 1.12.2 MCP 类和调用链
2. 检查现有 Rust 实现,避免重复重写
3. 使用 Rust 等价结构实现原版语义
4. 不引入假数据、占位几何或静态截图
5. 保持网络、GUI、渲染层和状态顺序
6. 同时检查 Android 与 Windows 构建
7. 运行格式、Release 检查和测试
8. 在真实客户端场景中进行对照验证

## 法律声明

- 本项目与 Mojang Studios、Microsoft、MCP、OptiFine 均无官方隶属或认可关系。
- “Minecraft”及相关资产归其权利人所有。
- 本仓库不授予任何 Minecraft、MCP、OptiFine、RustCraft、Exhibition-Reborn、光影包或材质包的再分发权。
- 账号管理器的交互与行为参考 Exhibition-Reborn；仓库不包含其原始 Java 二进制、专有资源或品牌资产。
- 使用者必须自行拥有合法的 Minecraft 资源来源并遵守相关许可和服务条款。
- 仓库代码的使用权限以根目录 `LICENSE` 为准。

<img width="1920" height="1020" alt="QQ20260810-113451" src="https://github.com/user-attachments/assets/8fae411f-57e8-4885-ac72-a2be00c98538" />
<img width="1920" height="1020" alt="QQ20260810-113523" src="https://github.com/user-attachments/assets/c7bb67a2-85ec-452d-bce5-7aa01278c748" />
<img width="1920" height="1020" alt="QQ20260810-113639" src="https://github.com/user-attachments/assets/e1209daa-c777-40be-b244-2df2ed6ee1ff" />
<img width="1920" height="1020" alt="QQ20260810-113851" src="https://github.com/user-attachments/assets/07e50b4b-2a7b-4798-8877-77558c29da0b" />
随便传几张照片展示下效果罢了
