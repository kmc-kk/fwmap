# fwmap マニュアル

## 1. 概要

`fwmap` は、組込みファームウェアの `ELF` と GNU ld / LLVM lld 系 `map` を解析し、ROM/RAM 使用量、主要シンボル、object 寄与、前回ビルドとの差分を可視化するローカル CLI ツールです。

現行版は、単純なサイズ表示だけでなく、差分原因の追跡、memory region の可視化、JSON 出力、CI 向け要約、ルールベースの warning 判定、外部ルール設定、C++ symbol の demangle を主機能として扱います。

さらに現行版では、`gimli` を用いた DWARF line table 読み込みにより、source file / function / line-range 単位の attribution を扱えます。

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
- GNU ld / LLVM lld 系 map
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
- 外部 TOML ルール設定
- C++ symbol demangle
- DWARF line table 解析
- source file / function / line-range 集計
- SQLite ベースの履歴保存とトレンド表示
- `--toolchain auto|gnu|lld|iar|armcc|keil`
- `--verbose` / `--version`

現時点で未対応または限定的な内容:

- demangle の高度化
- linker script の完全構文対応
- 外部ルール設定の高度化
- split DWARF (`.dwo` / `.dwp`)

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
  --toolchain auto \
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

### 外部ルール設定を読み込む

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --rules tests/fixtures/sample_rules.toml
```

### C++ symbol を demangle して表示する

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on
```

### DWARF から source line を読む

```bash
fwmap analyze \
  --elf build/app.elf \
  --dwarf=auto \
  --source-lines lines \
  --source-root . \
  --path-remap build=src \
  --report-json build/fwmap_sources.json
```

### DWARF から source file / function / hotspot を出す

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on \
  --dwarf=on \
  --source-lines all \
  --out build/fwmap_sources.html
```

### map parser family を明示する

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --toolchain lld
```

### CI 向けに短い要約だけを出す

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-format markdown \
  --ci-source-summary \
  --max-source-diff-items 8 \
  --min-line-diff-bytes 64 \
  --ci-out build/fwmap_ci.md
```

### warning が出たら失敗にする

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --fail-on-warning
```

### 履歴を保存する

```bash
fwmap history record \
  --db history.db \
  --elf build/app.elf \
  --map build/app.map \
  --meta commit=abc123 \
  --meta branch=main
```

### 履歴一覧を表示する

```bash
fwmap history list --db history.db
```

### 特定ビルドの履歴詳細を表示する

```bash
fwmap history show --db history.db --build 1
```

### トレンドを表示する

```bash
fwmap history trend --db history.db --metric rom --last 20
fwmap history trend --db history.db --metric source:src/main.cpp --last 20
fwmap history trend --db history.db --metric function:src/main.cpp::_ZN3app4mainEv --last 20
fwmap history trend --db history.db --metric directory:src/app --last 20
fwmap history trend --db history.db --metric unknown_source --last 20
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

### 5.3.1 toolchain を自動判定する

`map` が GNU ld 由来か LLVM lld 由来か分からない場合は `--toolchain auto` を付けます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --toolchain auto \
  --out build/fwmap_report.html
```

確認ポイント:

1. 標準出力の `Toolchain` 行を見る
2. HTML の `Overview` で resolved toolchain を見る
3. JSON の `toolchain` を CI や後続スクリプトで確認する

GNU ld と LLVM lld は同じ内部モデルに正規化されるため、以後の `Top Object Contributions` や `Diff` の見方は同じです。

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
- source files
- functions
- line hotspots
- source diff
- regions
- diff summary
- diff 本体

具体例:

- CI の後段で `fwmap_report.json` を読んで集計する
- warning 件数を別ツールで通知する
- `diff_summary` を使って増減件数だけダッシュボード化する

### 5.7 CI 向けの短い要約を出す

ログや PR コメント向けに短い要約を出したい場合は `--ci-format` を使います。`--ci-summary` は text 形式の簡易指定としてそのまま使えます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-format markdown \
  --ci-out build/fwmap_ci.md
```

出力例:

```text
ROM: +12345 bytes
RAM: +2048 bytes
Warnings: 2
Errors: 1
Top section growth: .bss (+8192)
Top symbol growth: g_rx_ring (+4096)
Triggered rules: forbid-g-rx-ring(error), REGION_THRESHOLD(warn)
```

用途:

- GitHub Actions のログを短く保つ
- GitLab CI の job log で一画面に収める
- 人がまず差分だけを素早く確認する
- markdown を PR / MR コメントへそのまま貼る
- JSON を別ジョブで機械判定する
- source file / function / line-range の増加要因を短く共有する

利用できる形式:

- `text`
- `markdown`
- `json`

source diff を CI に含めたい場合:

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-format markdown \
  --ci-source-summary \
  --max-source-diff-items 8 \
  --min-line-diff-bytes 64 \
  --ci-out build/fwmap_ci.md
```

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
- error severity の rule が 1 件以上あれば終了コード 2
- `--fail-on-warning` 指定時に warn / info の warning があれば終了コード 1

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

### 5.9.1 外部ルールファイルを使う

`--rules <path>` で TOML 形式のルール設定ファイルを読み込めます。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --rules tests/fixtures/sample_rules.toml
```

最小例:

```toml
schema_version = 1

[thresholds]
rom_usage_warn = 0.90
ram_usage_warn = 0.88

[[rules]]
id = "flash-near-full"
kind = "region_usage"
region = "FLASH"
warn_if_greater_than = 0.92
severity = "warn"
message = "FLASH usage is above 92%"

[[rules]]
id = "tls-data-growth"
kind = "section_delta"
section = ".data"
warn_if_delta_bytes_gt = 2048
severity = "warn"
message = ".data increased by more than 2KB"
```

この設定でできること:

- 内蔵しきい値を TOML から上書きする
- 特定 region の使用率に独自 rule を足す
- 特定 section の増加量に独自 rule を足す
- 特定 source path 配下の増加を検知する
- 特定 function 名パターンの増加を検知する
- unknown source ratio を警告化する

現行版で対応している `kind`:

- `region_usage`
- `section_delta`
- `symbol_delta`
- `symbol_match`
- `object_match`
- `source_path_growth`
- `function_growth`
- `unknown_source_ratio`

ルールファイルが壊れている場合や必須項目が足りない場合は、解析前に明確なエラーで停止します。

### 5.9.2 demangle の使い分け

`--demangle=auto|on|off` を使えます。

- `auto`: Itanium ABI らしい名前だけ demangle を試す
- `on`: demangle を積極的に試す
- `off`: 生シンボル名のまま表示する

例:

```bash
fwmap analyze --elf build/app.elf --map build/app.map --demangle=auto
fwmap analyze --elf build/app.elf --map build/app.map --demangle=on
fwmap analyze --elf build/app.elf --map build/app.map --demangle=off
```

`diff` や内部比較は raw symbol 名を使い、表示だけ demangled 名を優先します。そのため、前回比較のキーが崩れることはありません。

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

### 5.12 履歴を保存して継続監視する

サイズ推移を継続監視したい場合は `history` サブコマンドを使います。

```bash
fwmap history record \
  --db history.db \
  --elf build/app.elf \
  --map build/app.map \
  --meta commit=abc123 \
  --meta branch=main
```

これで 1 回分の解析結果を SQLite に保存できます。

一覧確認:

```bash
fwmap history list --db history.db
```

特定ビルドの詳細確認:

```bash
fwmap history show --db history.db --build 3
```

推移確認:

```bash
fwmap history trend --db history.db --metric rom --last 20
fwmap history trend --db history.db --metric ram --last 20
fwmap history trend --db history.db --metric warnings --last 20
fwmap history trend --db history.db --metric unknown_source --last 20
fwmap history trend --db history.db --metric region:FLASH --last 20
fwmap history trend --db history.db --metric section:.bss --last 20
fwmap history trend --db history.db --metric source:src/main.cpp --last 20
fwmap history trend --db history.db --metric function:src/main.cpp::_ZN3app4mainEv --last 20
fwmap history trend --db history.db --metric directory:src/app --last 20
```

使いどころ:

- 毎日の build で ROM / RAM 推移を残す
- 特定 region の悪化傾向を追う
- `.bss` や `.data` の長期増加を確認する
- warning 件数の推移を監視する
- 特定 source file の増減を継続監視する
- 変動の激しい function を追う
- unknown source ratio の悪化を検知する

`history show` では以下も確認できます。

- DWARF 利用有無
- unknown source ratio
- 上位 source files
- 上位 functions

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
| `--rules <path>` | 任意 | 外部 TOML ルール設定 |
| `--demangle=auto|on|off` | 任意 | C++ symbol demangle 制御 |
| `--toolchain <auto|gnu|lld|iar|armcc|keil>` | 任意 | map parser family の自動判定または強制指定 |
| `--dwarf=auto|on|off` | 任意 | DWARF line table の使用有無 |
| `--source-lines <off|files|functions|lines|all>` | 任意 | source attribution の粒度 |
| `--source-root <path>` | 任意 | 相対 source path に付けるルート |
| `--path-remap <from=to>` | 任意 | DWARF source path の prefix remap。複数指定可 |
| `--fail-on-missing-dwarf` | 任意 | DWARF 必須時に欠落をエラー化 |
| `--ci-summary` | 任意 | CI 向けの短い要約を表示 |
| `--ci-format <text|markdown|json>` | 任意 | CI 要約の出力形式 |
| `--ci-out <path>` | 任意 | CI 要約の出力先 |
| `--ci-source-summary` | 任意 | source file / function / line-range diff を CI 要約へ含める |
| `--max-source-diff-items <n>` | 任意 | source diff の表示件数上限 |
| `--min-line-diff-bytes <n>` | 任意 | 小さすぎる line diff を省略するしきい値 |
| `--hide-unknown-source` | 任意 | unknown source diff を要約から隠す |
| `--fail-on-warning` | 任意 | warning があれば非 0 終了 |
| `--threshold-rom <percent>` | 任意 | ROM warning しきい値 |
| `--threshold-ram <percent>` | 任意 | RAM warning しきい値 |
| `--threshold-region <name:percent>` | 任意 | region ごとの warning しきい値 |
| `--threshold-symbol-growth <bytes>` | 任意 | symbol growth warning しきい値 |
| `--verbose` | 任意 | warning 詳細を標準出力へ表示 |
| `--version` | 任意 | バージョン表示 |
| `--help` | 任意 | ヘルプ表示 |

履歴サブコマンド:

| コマンド | 説明 |
| --- | --- |
| `history record --db <path> --elf <path>` | 履歴を 1 件保存 |
| `history list --db <path>` | 保存済み履歴の一覧表示 |
| `history show --db <path> --build <id>` | 特定 build の詳細表示 |
| `history trend --db <path> --metric <metric>` | 推移表示 |

## 7. 出力内容

`fwmap` は次の 3 種類を出力できます。

- 標準出力: 実行サマリ
- HTML: 単一ファイルのレポート
- JSON: 機械可読なレポート
- CI summary: text / markdown / JSON の短い要約
- History: SQLite に保存した履歴の一覧・詳細・推移

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

markdown あり:

```markdown
# fwmap CI Summary

| Metric | Value |
| --- | --- |
| ROM delta | +12345 bytes |
| RAM delta | +2048 bytes |
| Warnings | 2 |
| Errors | 1 |
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
- `debug_info`
- `top_symbols`
- `top_object_contributions`
- `archive_contributions`
- `source_files`
- `functions`
- `line_hotspots`
- `line_attributions`
- `unknown_source`
- `source_diff`
- `regions`
- `diff_summary`
- `diff`

`top_symbols` の各要素は、raw 名の `name` と表示用の `demangled_name` を両方持ちます。

`toolchain` には、ユーザ指定、検出結果、実際に使った parser family が入ります。

## 8. HTML レポートの見方

HTML は以下の順で構成されます。

1. Header
2. Overview
3. Warnings
4. Source Summary
5. Source Files
6. Top Functions
7. Line Hotspots
8. Memory Summary
9. Memory Regions Overview
10. Region Sections
11. Section Breakdown
12. Top Symbols
13. Top Object Contributions
14. Diff
15. Footer

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

warning 判定はルール単位で分離され、さらに外部 TOML ルールを読み込めるため、同じ `code` を軸に CLI、HTML、JSON を横断して追跡しやすくなっています。

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

### 8.3 Source Summary / Source Files / Top Functions / Line Hotspots

DWARF が使われた場合は、source 系のセクションが追加されます。

確認できる内容:

- `Source Summary`: compilation unit 数と unknown ratio
- `Source Files`: ファイル別の寄与サイズ、ディレクトリ、関数数
- `Top Functions`: symbol と source range を結び付けた関数別ランキング
- `Line Hotspots`: 連続または近接する行を圧縮した line-range
- `Trend Links`: `history trend` にそのまま使える metric 例

HTML 上の補助操作:

- Source Files / Top Functions / Line Hotspots の検索
- region / section / source path の絞り込み
- 長い path の短縮表示と hover での完全表示
- 行範囲 row への anchor

具体例:

- `src/net/tcp.cpp` が 12 KiB
- `net::TcpSession::poll()` が 4 KiB
- `src/net/tcp.cpp:120-134` が 2 KiB

追い方の目安:

1. まず `Source Files` で大きいファイルを探す
2. 次に `Top Functions` で関数へ絞る
3. 最後に `Line Hotspots` で line-range を確認する

### 8.4 Memory Summary

section をサイズ順に表示し、次のいずれかに分類します。

- `ROM`
- `RAM`
- `Other`

現行の集計ルール:

- ROM: `.text`, `.rodata`, read-only / executable な `ALLOC` section
- RAM: `.data`, `.bss`, writable な `ALLOC` section

### 8.5 Memory Regions Overview

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

### 8.6 Region Sections

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
- demangled 名
- section 名
- object 名
- サイズ

注意:

- symbol table が無い ELF ではこの情報は空になります
- その場合でも section summary と HTML 生成は継続します
- demangle が有効な場合は読みやすい名前を優先表示し、生の名前も保持します

### 8.9 Top Object Contributions

map がある場合に object ごとの寄与サイズ上位を表示します。

これにより、どの object が増加に効いているかを追えます。

### 8.10 Diff

差分確認で最も重要なセクションです。

表示内容:

- ROM 差分
- RAM 差分
- section 変化件数
- symbol 変化件数
- object 変化件数
- 上位増加 section
- 上位増加 symbol
- 上位増加 object
- 上位増加 source file
- 上位増加 function
- 上位増加 line-range
- unknown source delta
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

### 11.5 外部ルールで特定 symbol を禁止する

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --rules tests/fixtures/sample_rules.toml
```

用途:

- 特定 symbol を含む build を拒否する
- 特定 section 増加量にローカル運用ルールを足す
- チーム固有の region 制約を追加する

### 11.6 C++ プロジェクトで見やすくする

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on
```

用途:

- `_ZN...` 形式の symbol を人が読める形にする
- diff 上位 symbol の意味を把握しやすくする
- HTML / JSON / CLI の表示名を揃える

### 11.7 長期トレンドを確認する

```bash
fwmap history trend --db history.db --metric rom --last 20
```

用途:

- 直近 20 build の ROM 推移を確認する
- Release 前に長期的な増加傾向を確認する
- warning 件数や特定 section の増加を追う

## 12. 内部構成

現行の主要モジュール:

- `cli`: 引数処理と実行制御
- `core`: 解析、モデル、diff、rules、history などの中核ロジック
- `ingest`: ELF / map / linker script の読み込み
- `report`: CLI / HTML / JSON / CI 出力
- `ingest/dwarf`: gimli ベースの DWARF line table 読み込み
- `validation`: 解析後の整合性チェック
- `docs/toolchains.md`: toolchain family と parser 追加手順

主要ファイル:

- CLI: [src/cli/mod.rs](/e:/work/git/fwmap/src/cli/mod.rs)
- ELF parser: [src/ingest/elf/mod.rs](/e:/work/git/fwmap/src/ingest/elf/mod.rs)
- map parser: [src/ingest/map/mod.rs](/e:/work/git/fwmap/src/ingest/map/mod.rs)
- linker script parser: [src/ingest/linker/mod.rs](/e:/work/git/fwmap/src/ingest/linker/mod.rs)
- dwarf parser: [src/ingest/dwarf/mod.rs](/e:/work/git/fwmap/src/ingest/dwarf/mod.rs)
- analyze: [src/core/analyze.rs](/e:/work/git/fwmap/src/core/analyze.rs)
- rules: [src/core/rules.rs](/e:/work/git/fwmap/src/core/rules.rs)
- rule config: [src/core/rule_config.rs](/e:/work/git/fwmap/src/core/rule_config.rs)
- demangle: [src/core/demangle.rs](/e:/work/git/fwmap/src/core/demangle.rs)
- history: [src/core/history.rs](/e:/work/git/fwmap/src/core/history.rs)
- diff: [src/core/diff.rs](/e:/work/git/fwmap/src/core/diff.rs)
- model: [src/core/model.rs](/e:/work/git/fwmap/src/core/model.rs)
- render: [src/report/render.rs](/e:/work/git/fwmap/src/report/render.rs)
- quality checks: [src/validation/quality.rs](/e:/work/git/fwmap/src/validation/quality.rs)

## 13. 既知の制約

- ELF は現在 `SHT_SYMTAB` を中心に参照
- map は GNU ld と LLVM lld を正式対応
- object path は主に map 由来
- archive/member の表記揺れは主要ケース対応に留まる
- linker script は subset 対応であり、複雑な式や完全構文には未対応
- region 使用量は linker script と ELF section address を組み合わせた推定を含む
- JSON schema は現時点で `schema_version = 1`
- ROM/RAM はヒューリスティック集計
- demangle は現在 Itanium ABI 系の軽量対応
- 外部ルール設定は TOML 固定で、対応 `kind` は現在の実装範囲に限られる
- 履歴保存はローカル SQLite 前提で、現時点では CLI 表示中心
- `--toolchain auto` の検出は軽量判定であり、現時点では GNU ld / LLVM lld の主要パターンに限定
- DWARF attribution は line table と ELF symbol range の組み合わせで集計している
- 最適化ビルドでは line attribution は近似的であり、source order と一致しない場合がある
- split DWARF (`.dwo` / `.dwp`) は未対応

## 14. 今後の予定

今後の主な候補:

- CI 出力強化
- demangle の高度化
- 履歴トレンド
- 対応 toolchain の追加

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
- `discarded_sections.map`
- `non_ascii.map`
- `sample_rules.toml`
- `sample.ld`
- `sample_lld.map`

ELF の一部フィクスチャはテスト内で合成生成しています。
