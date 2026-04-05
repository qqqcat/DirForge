# DirOtter 打包指南（Linux + macOS）

## Linux 打包（tar.gz）

在 Linux 主机执行：

```bash
./scripts/package-linux.sh
```

输出：

- `dist/linux/DirOtter-<version>-linux-x86_64/`
- `dist/linux/DirOtter-<version>-linux-x86_64.tar.gz`

可通过环境变量覆盖版本号：

```bash
DIROTTER_VERSION=0.1.0 ./scripts/package-linux.sh
```

## macOS 打包（.app + zip）

在 macOS 主机执行：

```bash
./scripts/package-macos.sh
```

输出：

- `dist/macos/DirOtter.app`
- `dist/macos/DirOtter-<version>-macos.zip`

可通过环境变量覆盖版本号：

```bash
DIROTTER_VERSION=0.1.0 ./scripts/package-macos.sh
```

> 说明：该脚本仅生成可运行的 `.app` 包结构与 zip，不包含 Apple notarization/signing。正式分发前请追加签名与公证流程。
