# Minecraft Java Edition 1.12.2 — Rust Semantic Port (Android)

> 使用 Rust 对 Minecraft Java Edition 1.12.2 客户端进行语义级移植,当前重心为 **Android 平台(协议 340,Vulkan 渲染,物理键鼠操作)**。

当前 Android 基线:**0.112.1**(同步上游 v0.112.1)<br>
验证设备:**Xiaomi 12 Pro**(Adreno 730,Android 12+,Vulkan 1.1)<br>
桌面基线:**Windows 10/11 x64**(Vulkan / OpenGL 双后端,见[桌面版章节](#桌面版-windows))

---

## 注意!!!

项目需要导入 1.12.2 资源。手动导入十分复杂,推荐使用自动导入脚本,但自动导入脚本不一定能成功运行,这主要取决于你的环境与目标资源文件。可以使用本机的 1.12.2 资源,但推荐使用官方重写参照的 MCP-1.12.2-main.zip,这个自动导入资源脚本成功运行率更高:https://1850640083.share.123pan.cn/123pan/RvAxvd-NIOgA

## 项目简介

本项目的目标不是制作一个"外观类似 Minecraft"的独立仿制游戏,而是在 Rust 中尽可能忠实地重建 Minecraft Java Edition 1.12.2 客户端的行为、状态、调用流程、网络协议、GUI、模型、动画和操作体验。原版 Minecraft 1.12.2 与对应 MCP 代码结构是行为基准。

**Android 版定位**:远程多人客户端,Vulkan 渲染,物理键鼠为主,触屏通过桥接层映射为键鼠操作(点击/滑动/长按)。项目本身不含单人集成服务器。

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

- 项目不是 Mojang 官方客户端
- 主要运行路径是远程多人客户端;单人集成服务器不在当前基线
- 部分少见实体、TileEntity、交互或视觉边缘情况仍可能与原版存在差异
- 任意第三方 OptiFine 光影包的普遍兼容性未作保证
- 不捆绑原版资产,首次构建前必须导入资源
- 发现差异时,请提供:原版表现、本项目表现、可复现步骤、渲染后端、资源包/光影包、完整日志

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

- 本项目与 Mojang Studios、Microsoft、MCP、OptiFine 均无官方隶属或认可关系
- "Minecraft"及相关资产归其权利人所有
- 本仓库不授予任何 Minecraft、MCP、OptiFine、光影包或材质包的再分发权
- 使用者必须自行拥有合法的 Minecraft 资源来源并遵守相关许可和服务条款
- 仓库代码的使用权限以根目录 `LICENSE` 为准
