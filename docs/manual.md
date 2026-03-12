# fwmap マニュアル

## 1. 概要

`fwmap` は、組込みファームウェアの `ELF` と GNU ld 系 `map` を解析し、ROM/RAM 使用量、主要シンボル、object 寄与、前回ビルドとの差分を可視化するローカル CLI ツールです。

現行版は Phase 6 まで実装済みで、単純なサイズ表示だけでなく、差分原因の追跡、memory region の可視化、JSON 出力、CI 向け要約、ルールベースの warning 判定を主機能として扱います。

このツールで把握しやすい内容:

- どの section が ROM / RAM を使っているか
- どの symbol が大きいか
- どの object file がサイズに効いているか
- 前回ビルドから何が増えたか、減ったか
- 追加・削除・増加・減少のどれに当たるか
- linker script 上の region と section 配置がどうなっているか
- CI で機械判定できる JSON と短い要約を得る

## 2. 対応範囲

- ELF32 / ELF64
- little-endian ELF を主対象
- GNU ld 系 map
- 単一 HTML レポート出力
- 前回成果物との diff 比較
- GNU ld linker script subset 解析
- memory region overview
- section と region の対応表示
- JSON 出力
- CI 向け要約出力
- warning ベースの終了コード制御
- しきい値カスタマイズ
- ルールベースの warning 判定
- `--verbose` / `--version`

現時点で未対応または限定的な内容:

- demangle 改善
- DWARF を用いたソース行解析
- linker script の完全構文対応
- 外部ルール設定ファイル

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

### ELF / map / linker script を合わせて解析

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
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

### JSON レポートを出力

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --report-json build/fwmap_report.json
```

### CI 向けに短い要約だけを出す

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-summary
```

### warning が出たら失敗にする

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --fail-on-warning
```

## 5. 実際の使用手順

この章では、日常的な使い方を具体的な流れで説明します。

### 5.1 事前準備

最低限そろえるファイル:

- 現在のビルド成果物 `app.elf`
- 可能なら `app.map`
- region 可視化を使う場合は linker script `app.ld`
- 差分比較する場合は前回ビルドの `prev_app.elf` と `prev_app.map`

典型的な配置例:

```text
project/
  build/
    app.elf
    app.map
  linker/
    app.ld
  prev/
    app.elf
    app.map
```

### 5.1.1 CI で使う場合に追加で考えること

CI で使う場合は以下も決めておくと運用しやすくなります。

- HTML だけ保存するか
- JSON も artifact として保存するか
- warning を失敗扱いにするか
- しきい値をどこまで厳しくするか

### 5.2 まず ELF だけで確認する

最初に `ELF` だけで全体像を見ます。

```bash
fwmap analyze --elf build/app.elf
```

この段階で分かること:

- section 数
- ROM / RAM の概算
- 大きい symbol
- 基本 warning

向いている場面:

- ビルド直後のざっくり確認
- map が未生成のビルド環境
- とりあえず HTML を出したいとき

### 5.3 map を付けて object 寄与を確認する

object ごとの寄与を確認したい場合は `--map` を付けます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --out build/fwmap_report.html
```

実行後の確認ポイント:

1. HTML の `Top Object Contributions` を開く
2. サイズの大きい object を確認する
3. `Top Symbols` と照らし合わせて、どの object に大きい symbol が入っているかを見る

具体例:

- `drivers/net.o` が大きい
- `g_rx_ring` が大きい
- その結果 `.bss` が増えている

この場合は、ネットワークバッファやキュー定義を疑うのが自然です。

### 5.4 linker script を付けて region 配置を確認する

memory region を確認したい場合は `--lds` を付けます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
  --out build/fwmap_region_report.html
```

実行後の確認ポイント:

1. `Memory Regions Overview` を見る
2. `FLASH` や `RAM` の使用率を確認する
3. `Region Sections` で、その region に載っている section を見る
4. warning に `REGION_THRESHOLD` や `SECTION_REGION_MISMATCH` が出ていないか確認する

具体例:

- `FLASH` 使用率が 91%
- `RAM` free が 2 KiB
- `.data` が `RAM` に載る想定だが、section address が region 範囲外

この場合は、linker script の割当、section 属性、または section address 解釈を優先的に確認します。

### 5.5 前回ビルドとの差分を確認する

サイズ回帰を追う場合は、前回成果物を与えます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --out build/fwmap_diff_report.html
```

標準出力の例:

```text
ELF: build/app.elf
ROM: 65536 bytes (64.00 KiB) | RAM: 16384 bytes (16.00 KiB) | Sections: 30 | Symbols: 240 | Warnings: 2
ROM: +3072 bytes
RAM: +1024 bytes
Diff counts: sections +1 / -0 / ↑4 / ↓1, symbols +3 / -1 / ↑7 / ↓2
Top growth symbol: app_tls_buffer (+1024)
Top growth object: middleware/tls.o (+1536)
Report: build/fwmap_diff_report.html
```

この出力からの読み方:

1. まず `ROM` / `RAM` の増加量を見る
2. 次に `Top growth symbol` を見る
3. さらに `Top growth object` を見る
4. HTML の `Diff` セクションで `Added` / `Removed` / `Increased` を確認する

### 5.6 JSON を出力して CI やスクリプトで使う

JSON を出すと、人が HTML を見るだけでなく、機械的な判定や後処理がしやすくなります。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
  --report-json build/fwmap_report.json
```

出力される主な内容:

- binary metadata
- linker script 情報
- section summary
- memory summary
- warnings
- thresholds
- top symbols
- top object contributions
- regions
- diff summary
- diff 本体

具体例:

- CI の後段で `fwmap_report.json` を読んで集計する
- warning 件数を別ツールで通知する
- `diff_summary` を使って増減件数だけダッシュボード化する

### 5.7 CI 向けの短い要約を出す

ログを短く保ちたい場合は `--ci-summary` を使います。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-summary
```

出力例:

```text
ROM: +12345 bytes
RAM: +2048 bytes
Warnings: 2
Top section growth: .bss (+8192)
Top symbol growth: g_rx_ring (+4096)
```

用途:

- GitHub Actions のログを短く保つ
- GitLab CI の job log で一画面に収める
- 人がまず差分だけを素早く確認する

### 5.8 warning が出たら job を失敗にする

warning を見逃したくない場合は `--fail-on-warning` を付けます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --fail-on-warning
```

挙動:

- warning が 0 件なら終了コード 0
- warning が 1 件以上なら非 0

使いどころ:

- サイズ劣化を CI で即座に止めたいとき
- Release build だけ厳しく判定したいとき

### 5.9 しきい値を調整する

しきい値は CLI から変更できます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --threshold-rom 90 \
  --threshold-ram 90 \
  --threshold-region FLASH:92 \
  --threshold-symbol-growth 8192
```

この例の意味:

- ROM は 90% から warning
- RAM は 90% から warning
- `FLASH` は 92% から warning
- symbol growth は 8192 bytes 以上で warning

CI では、開発初期は緩く、リリース前は厳しくすると運用しやすいです。

### 5.10 warning を詳しく見たい場合

標準出力に warning を詳細表示したい場合:

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
  --verbose
```

出力例:

```text
Warnings:
  [analyze:REGION_THRESHOLD] Region FLASH usage exceeded 85% (91.2%)
  [analyze:REGION_LOW_FREE] Region RAM free space is low (2048 bytes (2.00 KiB))
  [analyze:LARGE_SYMBOL] Large symbol detected: g_rx_ring (12288 bytes (12.00 KiB))
  [map:MAP_LINE_SKIPPED] Skipped unparsed map line 120: COMMON ...
```

この情報で分かること:

- `source`: どこで出た warning か
- `code`: warning 種別
- `message`: 具体的な理由
- `related`: どの region / section / symbol に紐づくか

### 5.11 HTML の保存先を分ける

複数条件でレポートを比較したい場合は、出力先を分けます。

```bash
fwmap analyze --elf build/app.elf --out reports/elf_only.html
fwmap analyze --elf build/app.elf --map build/app.map --out reports/with_map.html
fwmap analyze --elf build/app.elf --map build/app.map --lds linker/app.ld --out reports/with_regions.html
fwmap analyze --elf build/app.elf --map build/app.map --prev-elf prev/app.elf --prev-map prev/app.map --out reports/diff.html
```

これで:

- ELF のみ
- map 付き
- region 付き
- diff 付き

を横並びで比較できます。

### 5.12 外部ルール設定の書き方

現行版では、外部ファイルから warning ルールを読み込む機能はまだ実装されていません。

ただし Phase 6 で warning 判定は `rules` モジュールに分離されており、将来的には外部設定ファイルを読み込める形へ拡張する前提で整理されています。この節では、将来の設定ファイルを設計する際の推奨イメージを示します。

想定する考え方:

- 1 つの rule に 1 つの `code` を対応させる
- `severity` は `info` / `warn` / `error` を想定する
- `enabled` で個別 rule の有効・無効を切り替える
- `threshold` や `entity` で rule ごとの条件を持たせる

設定ファイルの想定例:

```toml
schema_version = 1

[[rules]]
code = "ROM_THRESHOLD"
enabled = true
severity = "warn"
threshold_percent = 90

[[rules]]
code = "RAM_THRESHOLD"
enabled = true
severity = "warn"
threshold_percent = 90

[[rules]]
code = "REGION_THRESHOLD"
enabled = true
severity = "warn"
entity = "FLASH"
threshold_percent = 92

[[rules]]
code = "REGION_LOW_FREE"
enabled = true
severity = "warn"
entity = "RAM"
threshold_bytes = 4096

[[rules]]
code = "LARGE_SYMBOL"
enabled = true
severity = "warn"
threshold_bytes = 8192

[[rules]]
code = "SYMBOL_SPIKE"
enabled = true
severity = "warn"
threshold_bytes = 8192
```

この例の意味:

- `ROM_THRESHOLD`: ROM 使用率が 90% を超えたら warning
- `RAM_THRESHOLD`: RAM 使用率が 90% を超えたら warning
- `REGION_THRESHOLD`: `FLASH` の使用率が 92% を超えたら warning
- `REGION_LOW_FREE`: `RAM` の空きが 4096 bytes 以下になったら warning
- `LARGE_SYMBOL`: 8192 bytes 以上の symbol を warning
- `SYMBOL_SPIKE`: 前回比で 8192 bytes 以上増えた symbol を warning

現行 CLI との対応関係:

| 将来の外部 rule | 現行 CLI の指定方法 |
| --- | --- |
| `ROM_THRESHOLD` | `--threshold-rom <percent>` |
| `RAM_THRESHOLD` | `--threshold-ram <percent>` |
| `REGION_THRESHOLD` | `--threshold-region <name:percent>` |
| `SYMBOL_SPIKE` | `--threshold-symbol-growth <bytes>` |

未対応の点:

- `LARGE_SYMBOL` のしきい値を CLI から変更する機能
- `DATA_GROWTH` / `BSS_GROWTH` を外部設定から変更する機能
- `severity` の外部変更
- rule の完全な enable / disable

運用上は、いま使える設定は CLI オプションで管理し、将来外部設定へ移したときに同じ `code` を継続利用できるようにしておくのが安全です。

## 6. CLI オプション

| オプション | 必須 | 説明 |
| --- | --- | --- |
| `--elf <path>` | 必須 | 現在の ELF ファイル |
| `--map <path>` | 任意 | 現在の GNU ld map ファイル |
| `--lds <path>` | 任意 | GNU ld linker script |
| `--prev-elf <path>` | 任意 | 比較用の前回 ELF |
| `--prev-map <path>` | 任意 | 比較用の前回 map |
| `--out <path>` | 任意 | HTML 出力先 |
| `--report-json <path>` | 任意 | JSON 出力先 |
| `--ci-summary` | 任意 | CI 向けの短い要約を表示 |
| `--fail-on-warning` | 任意 | warning があれば非 0 終了 |
| `--threshold-rom <percent>` | 任意 | ROM warning しきい値 |
| `--threshold-ram <percent>` | 任意 | RAM warning しきい値 |
| `--threshold-region <name:percent>` | 任意 | region ごとの warning しきい値 |
| `--threshold-symbol-growth <bytes>` | 任意 | symbol growth warning しきい値 |
| `--verbose` | 任意 | warning 詳細を標準出力へ表示 |
| `--version` | 任意 | バージョン表示 |
| `--help` | 任意 | ヘルプ表示 |

## 7. 出力内容

`fwmap` は次の 3 種類を出力できます。

- 標準出力: 実行サマリ
- HTML: 単一ファイルのレポート
- JSON: 機械可読なレポート

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

CI summary あり:

```text
ROM: +12345 bytes
RAM: +2048 bytes
Warnings: 2
Top section growth: .bss (+8192)
Top symbol growth: g_rx_ring (+4096)
```

### JSON の主な構造

JSON は固定 schema で出力されます。

主な top-level key:

- `schema_version`
- `binary`
- `linker_script`
- `section_summary`
- `memory_summary`
- `warnings`
- `thresholds`
- `top_symbols`
- `top_object_contributions`
- `archive_contributions`
- `regions`
- `diff_summary`
- `diff`

## 8. HTML レポートの見方

HTML は以下の順で構成されます。

1. Header
2. Overview
3. Warnings
4. Memory Summary
5. Memory Regions Overview
6. Region Sections
7. Section Breakdown
8. Top Symbols
9. Top Object Contributions
10. Diff
11. Footer

### 8.1 Overview

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

### 8.2 Warnings

warning は source と関連対象付きで表示されます。

Phase 6 以降は warning 判定がルール単位で分離されているため、同じ `code` を軸に CLI、HTML、JSON を横断して追跡しやすくなっています。

主な warning 例:

- ROM 使用率超過
- RAM 使用率超過
- 巨大 symbol 検出
- `.data` 増加
- `.bss` 増加
- symbol 急増
- region free space 低下
- section と region の不整合
- symbol table 欠損
- map の一部読み飛ばし

### 8.3 Memory Summary

section をサイズ順に表示し、次のいずれかに分類します。

- `ROM`
- `RAM`
- `Other`

現行の集計ルール:

- ROM: `.text`, `.rodata`, read-only / executable な `ALLOC` section
- RAM: `.data`, `.bss`, writable な `ALLOC` section

### 8.4 Memory Regions Overview

linker script がある場合に表示されます。

表示内容:

- region 名
- origin
- used
- free
- usage

用途:

- `FLASH` や `RAM` の逼迫状況を一目で確認する
- どの region から先に危険になるかを把握する

### 8.5 Region Sections

region ごとの section 一覧を表示します。

用途:

- どの section が `FLASH` / `RAM` / その他 region に載っているか確認する
- 想定外の配置を見つける

### 8.7 Section Breakdown

section ごとの詳細:

- 名前
- アドレス
- サイズ
- flags

メモリ配置の概況を目視確認する用途です。

### 8.8 Top Symbols

ELF symbol table からサイズ上位を表示します。

表示項目:

- symbol 名
- section 名
- object 名
- サイズ

注意:

- symbol table が無い ELF ではこの情報は空になります
- その場合でも section summary と HTML 生成は継続します

### 8.9 Top Object Contributions

map がある場合に object ごとの寄与サイズ上位を表示します。

これにより、どの object が増加に効いているかを追えます。

### 8.10 Diff

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

## 9. graceful degradation

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

## 10. エラーメッセージ

主なエラー:

- ELF file does not exist
- map file does not exist
- file is not an ELF
- unsupported ELF class / endianness
- ELF に section table が無い
- HTML report の書き込み失敗

CLI は panic を直接見せない方針です。原因候補が分かるように、可能な限り説明付きで返します。

## 11. 典型的な使い方

### 11.1 まず ELF だけ見る

```bash
fwmap analyze --elf build/app.elf
```

用途:

- ビルド直後の基本サイズ確認
- map がまだ無い場合の簡易確認

### 11.2 object 寄与を見る

```bash
fwmap analyze --elf build/app.elf --map build/app.map
```

用途:

- object 単位で増加要因を探す
- archive / member 由来のサイズ増加を見る

### 11.3 region 配置を見る

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld
```

用途:

- region 使用率を見る
- section と region の対応を確認する
- overflow に近い領域を早めに見つける

### 11.4 回帰調査をする

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

## 12. 内部構成

現行の主要モジュール:

- `cli`: 引数処理と実行制御
- `ingest`: ELF / map の読み込み
- `ingest/lds`: linker script subset 読み込み
- `analyze`: 集計と warning 判定
- `rules`: warning ルール評価
- `diff`: 差分計算と分類
- `model`: 共通データ構造
- `render`: CLI / HTML 出力

主要ファイル:

- CLI: [src/cli.rs](/e:/work/git/fwmap/src/cli.rs)
- ELF parser: [src/ingest/elf.rs](/e:/work/git/fwmap/src/ingest/elf.rs)
- map parser: [src/ingest/map.rs](/e:/work/git/fwmap/src/ingest/map.rs)
- linker script parser: [src/ingest/lds.rs](/e:/work/git/fwmap/src/ingest/lds.rs)
- analyze: [src/analyze.rs](/e:/work/git/fwmap/src/analyze.rs)
- rules: [src/rules.rs](/e:/work/git/fwmap/src/rules.rs)
- diff: [src/diff.rs](/e:/work/git/fwmap/src/diff.rs)
- render: [src/render.rs](/e:/work/git/fwmap/src/render.rs)

## 13. 既知の制約

- ELF は現在 `SHT_SYMTAB` を中心に参照
- map は GNU ld の典型出力を優先
- object path は主に map 由来
- archive/member の表記揺れは主要ケース対応に留まる
- linker script は subset 対応であり、複雑な式や完全構文には未対応
- region 使用量は linker script と ELF section address を組み合わせた推定を含む
- JSON schema は現時点で `schema_version = 1`
- ROM/RAM はヒューリスティック集計
- warning ルールは内部実装で固定されており、外部設定ファイル読み込みは未対応

## 14. 今後の予定

ロードマップ上の次候補:

- 外部ルール設定
- demangle 改善

## 15. テスト資産

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
- `sample.ld`

ELF の一部フィクスチャはテスト内で合成生成しています。
