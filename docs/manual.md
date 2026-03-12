# fwmap マニュアル

## 1. 概要

`fwmap` は、組込みファームウェアの `ELF` と GNU ld 系 `map` を解析し、ROM/RAM 使用量、主要シンボル、object 寄与、前回ビルドとの差分を可視化するローカル CLI ツールです。

現行版は Phase 3 まで実装済みで、単純なサイズ表示だけでなく、差分原因の追跡を主機能として扱います。

このツールで把握しやすい内容:

- どの section が ROM / RAM を使っているか
- どの symbol が大きいか
- どの object file がサイズに効いているか
- 前回ビルドから何が増えたか、減ったか
- 追加・削除・増加・減少のどれに当たるか

## 2. 対応範囲

- ELF32 / ELF64
- little-endian ELF を主対象
- GNU ld 系 map
- 単一 HTML レポート出力
- 前回成果物との diff 比較
- 固定しきい値ベースの warning
- `--verbose` / `--version`

現時点で未対応または限定的な内容:

- linker script 解析
- memory region ベース可視化
- JSON 出力
- CI 向け exit code 制御
- demangle 改善
- DWARF を用いたソース行解析

## 3. ビルドとテスト

前提:

- Rust toolchain
- Windows / Linux / macOS

ビルド:

```bash
cargo build
```

テスト:

```bash
cargo test
```

## 4. 基本的な使い方

### ELF のみ解析

```bash
fwmap analyze --elf build/app.elf
```

### ELF と map を解析

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --out report.html
```

### 前回ビルドとの差分を解析

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --out report.html
```

### バージョン表示

```bash
fwmap --version
```

### 詳細 warning を表示

```bash
fwmap analyze --elf build/app.elf --verbose
```

`--out` を省略した場合は `fwmap_report.html` が出力されます。

## 5. CLI オプション

| オプション | 必須 | 説明 |
| --- | --- | --- |
| `--elf <path>` | 必須 | 現在の ELF ファイル |
| `--map <path>` | 任意 | 現在の GNU ld map ファイル |
| `--prev-elf <path>` | 任意 | 比較用の前回 ELF |
| `--prev-map <path>` | 任意 | 比較用の前回 map |
| `--out <path>` | 任意 | HTML 出力先 |
| `--verbose` | 任意 | warning 詳細を標準出力へ表示 |
| `--version` | 任意 | バージョン表示 |
| `--help` | 任意 | ヘルプ表示 |

## 6. 出力内容

`fwmap` は次の 2 種類を出力します。

- 標準出力: 実行サマリ
- HTML: 単一ファイルのレポート

### 標準出力の例

通常時:

```text
ELF: build/app.elf
ROM: 32768 bytes (32.00 KiB) | RAM: 8192 bytes (8.00 KiB) | Sections: 24 | Symbols: 180 | Warnings: 1
Report: fwmap_report.html
```

diff あり:

```text
ELF: build/app.elf
ROM: 32768 bytes (32.00 KiB) | RAM: 8192 bytes (8.00 KiB) | Sections: 24 | Symbols: 180 | Warnings: 2
ROM: +1024 bytes
RAM: +256 bytes
Diff counts: sections +1 / -0 / ↑3 / ↓1, symbols +2 / -1 / ↑4 / ↓3
Top growth symbol: foo_bar (+512)
Top growth object: drivers/net.o (+768)
Report: fwmap_report.html
```

## 7. HTML レポートの見方

HTML は以下の順で構成されます。

1. Header
2. Overview
3. Warnings
4. Memory Summary
5. Section Breakdown
6. Top Symbols
7. Top Object Contributions
8. Diff
9. Footer

### 7.1 Overview

表示内容:

- 対象 ELF パス
- アーキテクチャ
- ELF class
- endian
- section 数
- ROM 合計
- RAM 合計
- warning 件数
- diff がある場合は ROM/RAM 差分

### 7.2 Warnings

warning は source と関連対象付きで表示されます。

主な warning 例:

- ROM 使用率超過
- RAM 使用率超過
- 巨大 symbol 検出
- `.data` 増加
- `.bss` 増加
- symbol 急増
- symbol table 欠損
- map の一部読み飛ばし

### 7.3 Memory Summary

section をサイズ順に表示し、次のいずれかに分類します。

- `ROM`
- `RAM`
- `Other`

現行の集計ルール:

- ROM: `.text`, `.rodata`, read-only / executable な `ALLOC` section
- RAM: `.data`, `.bss`, writable な `ALLOC` section

### 7.4 Section Breakdown

section ごとの詳細:

- 名前
- アドレス
- サイズ
- flags

メモリ配置の概況を目視確認する用途です。

### 7.5 Top Symbols

ELF symbol table からサイズ上位を表示します。

表示項目:

- symbol 名
- section 名
- object 名
- サイズ

注意:

- symbol table が無い ELF ではこの情報は空になります
- その場合でも section summary と HTML 生成は継続します

### 7.6 Top Object Contributions

map がある場合に object ごとの寄与サイズ上位を表示します。

これにより、どの object が増加に効いているかを追えます。

### 7.7 Diff

Phase 3 で最も強化されたセクションです。

表示内容:

- ROM 差分
- RAM 差分
- section 変化件数
- symbol 変化件数
- object 変化件数
- 上位増加 section
- 上位増加 symbol
- 上位増加 object
- Added Symbols
- Removed Symbols
- Removed Objects

差分分類:

- `Added`
- `Removed`
- `Increased`
- `Decreased`
- `Unchanged`
- `Moved`

現行版では `Moved` は将来拡張用の土台で、通常は主に Added / Removed / Increased / Decreased / Unchanged を使います。

## 8. graceful degradation

このツールは、解析できない箇所が一部あっても可能な範囲で処理を継続する設計です。

例:

- `--map` が無くても HTML は生成可能
- symbol table が無くても section summary は生成可能
- map に壊れた行があっても warning 化して継続

完全停止しやすいケース:

- ファイルが存在しない
- ELF ではない
- section table が存在しない
- HTML 出力先に書き込めない

## 9. エラーメッセージ

主なエラー:

- ELF file does not exist
- map file does not exist
- file is not an ELF
- unsupported ELF class / endianness
- ELF に section table が無い
- HTML report の書き込み失敗

CLI は panic を直接見せない方針です。原因候補が分かるように、可能な限り説明付きで返します。

## 10. 典型的な使い方

### 10.1 まず ELF だけ見る

```bash
fwmap analyze --elf build/app.elf
```

用途:

- ビルド直後の基本サイズ確認
- map がまだ無い場合の簡易確認

### 10.2 object 寄与を見る

```bash
fwmap analyze --elf build/app.elf --map build/app.map
```

用途:

- object 単位で増加要因を探す
- archive / member 由来のサイズ増加を見る

### 10.3 回帰調査をする

```bash
fwmap analyze \
  --elf current/app.elf \
  --map current/app.map \
  --prev-elf previous/app.elf \
  --prev-map previous/app.map \
  --out diff_report.html
```

用途:

- マージ後のサイズ退行確認
- ライブラリアップデート後の影響確認
- CI 導入前の手動差分調査

## 11. 内部構成

現行の主要モジュール:

- `cli`: 引数処理と実行制御
- `ingest`: ELF / map の読み込み
- `analyze`: 集計と warning 判定
- `diff`: 差分計算と分類
- `model`: 共通データ構造
- `render`: CLI / HTML 出力

主要ファイル:

- CLI: [src/cli.rs](/e:/work/git/fwmap/src/cli.rs)
- ELF parser: [src/ingest/elf.rs](/e:/work/git/fwmap/src/ingest/elf.rs)
- map parser: [src/ingest/map.rs](/e:/work/git/fwmap/src/ingest/map.rs)
- analyze: [src/analyze.rs](/e:/work/git/fwmap/src/analyze.rs)
- diff: [src/diff.rs](/e:/work/git/fwmap/src/diff.rs)
- render: [src/render.rs](/e:/work/git/fwmap/src/render.rs)

## 12. 既知の制約

- ELF は現在 `SHT_SYMTAB` を中心に参照
- map は GNU ld の典型出力を優先
- object path は主に map 由来
- archive/member の表記揺れは主要ケース対応に留まる
- linker script 非対応のため region 単位の厳密解析は未実装
- ROM/RAM はヒューリスティック集計

## 13. 今後の予定

ロードマップ上の次候補:

- Phase 4: linker script / memory region 対応
- Phase 5: JSON 出力と CI 連携
- Phase 6: ルールエンジン分離

## 14. テスト資産

`tests/fixtures/` には parser 回帰確認用の小さなサンプルがあります。

例:

- `sample.map`
- `broken.map`
- `archive_colon.map`
- `no_memory_config.map`
- `decimal_sizes.map`
- `tab_indented.map`
- `load_address.map`
- `unparsed_block.map`
- `mixed_case_regions.map`

ELF の一部フィクスチャはテスト内で合成生成しています。
