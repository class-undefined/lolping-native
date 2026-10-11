#!/usr/bin/env python3
"""Publish a complete, immutable build from main after every platform passes CI."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]


def gh(*args, **kwargs):
    return subprocess.run(["gh", *args], check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifacts", type=pathlib.Path)
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "native/gpui/Cargo.toml").read_text())["package"]["version"]
    commit = os.environ["GITHUB_SHA"]
    repository = os.environ["GITHUB_REPOSITORY"]
    run_number = os.environ["GITHUB_RUN_NUMBER"]
    current = gh("api", f"repos/{repository}/commits/main", "--jq", ".sha", capture_output=True).stdout.strip()
    if current != commit:
        print("A newer main commit exists; this run will not replace the latest release.")
        return
    expected = [f"lolPing-Native-{version}-arm64.{extension}" for extension in ("dmg", "zip")]
    for arch in ("x86_64", "aarch64"):
        expected += [f"lolPing-Native-{version}-windows-{arch}-setup.exe",
                     f"lolPing-Native-{version}-windows-{arch}.zip", f"verification-windows-{arch}.json"]
        report = json.loads((args.artifacts / f"verification-windows-{arch}.json").read_text())
        assert report["architecture"] == arch and report["commit"] == commit
        actual = hashlib.sha256((args.artifacts / report["installer"]).read_bytes()).hexdigest()
        assert actual == report["sha256"], "Installer differs from the verified binary"
    assets = [args.artifacts / name for name in expected]
    for asset in assets:
        if not asset.is_file() or asset.stat().st_size == 0:
            raise RuntimeError(f"Missing or empty release asset: {asset}")
    checksums = args.artifacts / "SHA256SUMS.txt"
    checksums.write_text("".join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in assets))
    assets.append(checksums)
    tag = f"v{version}+build.{run_number}"
    notes = args.artifacts / "release-notes.md"
    notes.write_text(f"""安装包由提交 `{commit}` 自动构建。三项平台任务全部通过后发布。

| 系统 | 下载文件 |
| --- | --- |
| Windows，Intel / AMD 64 位 | `*-windows-x86_64-setup.exe` |
| Windows，ARM64 / Snapdragon | `*-windows-aarch64-setup.exe` |
| macOS，Apple 芯片 | `*-arm64.dmg` |

Windows 安装包包含原生主程序、输入助手和全部素材，无需安装 Rust、Python、Node.js 或 Visual C++ 运行库。
两种 Windows 架构均在对应架构的运行器上完成安装、界面启动、输入助手、轮盘、编辑器、配置保存和卸载检查。
CI 不输出声音；音频文件完整性与 PCM 解码已检查。音频设备、游戏独占全屏和实际多屏环境仍受系统条件影响。

ZIP 为便携包，请完整解压后运行。macOS 将应用拖入 Applications，首次运行授予辅助功能权限。
安装包尚未进行 Windows 代码签名或 Apple 公证。

校验和见 `SHA256SUMS.txt`，Windows 验收明细见 `verification-windows-*.json`。
[使用说明](https://github.com/{repository}/blob/{commit}/README.md) · [构建记录](https://github.com/{repository}/actions/runs/{os.environ['GITHUB_RUN_ID']})
""", encoding="utf-8")
    existing = subprocess.run(["gh", "release", "view", tag, "--json", "isDraft"],
                              capture_output=True, text=True)
    if existing.returncode == 0:
        if not json.loads(existing.stdout)["isDraft"]:
            print(f"Release {tag} is already published; preserving its assets.")
            return
    else:
        gh("release", "create", tag, "--target", commit, "--draft", "--title",
           f"lolPing Native {version} · Build {run_number}", "--notes-file", str(notes))
    gh("release", "upload", tag, *map(str, assets), "--clobber")
    gh("release", "edit", tag, "--draft=false", "--latest", "--notes-file", str(notes))
    print(gh("release", "view", tag, "--json", "url", "--jq", ".url", capture_output=True).stdout)


if __name__ == "__main__":
    main()
