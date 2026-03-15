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
- SARIF 出力
- Why linked 説明
- CI 向け要約出力
- warning ベースの終了コード制御
- しきい値カスタマイズ
- ルールベースの warning 判定
- 外部 TOML ルール設定
- C++ symbol demangle
- C++ symbol classification and aggregate summaries
- DWARF line table 解析
- source file / function / line-range 集計
- SQLite ベースの履歴保存とトレンド表示
- `--toolchain auto|gnu|lld|iar|armcc|keil`
- `--map-format auto|gnu|lld-native`
- `--verbose` / `--version`

現時点で未対応または限定的な内容:

- demangle の高度化
- linker script の完全構文対応
- 外部ルール設定の高度化
- debuginfod の実ネットワーク取得

## 2.1 ROM / RAM サイズの見方

`fwmap` の ROM / RAM 集計は、ELF section の属性をもとにした runtime-oriented なヒューリスティックです。

- `ROM`: `.text`、`.rodata`、割り込みベクタ、その他の read-only / executable な `ALLOC` section
- `RAM`: `.data`、`.bss`、その他の writable な `ALLOC` section
- `ALLOC` を持たない section は、実行時メモリ使用量には含めません
- memory region の使用率は、linker script の region 定義と ELF section address を組み合わせて別途求めます
- `.debug_*` や `.zdebug_*` のようなデバッグ用 section は、実行時フットプリントに影響しないため、HTML の Memory Summary や Section Breakdown には含めません

つまり `fwmap` は単純に section 名だけで判定しているわけではなく、ELF の属性を優先して ROM / RAM に振り分けています。linker script を併用したときは、ROM / RAM 合計に加えて、実際にどの region に配置されたかを region 表示で確認できます。

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

### SARIF レポートを出力

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --sarif build/fwmap_report.sarif \
  --sarif-base-uri file:///workspace/ \
  --sarif-min-level warn
```

### なぜリンクされたかを確認する

```bash
fwmap explain \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
  --symbol main
```

archive member の採用理由を見る例:

```bash
fwmap explain \
  --elf build/app.elf \
  --map build/app.map \
  --object libapp.a(startup.o)
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

### C++ 集約サマリを CLI と JSON に出す

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on \
  --cpp-view \
  --report-json build/fwmap_cpp.json
```

このオプションを付けると、CLI に `Top template family` / `Top class` / `Runtime overhead` を短く出します。JSON には `cpp_view` が追加され、template family、class、method family、lambda group、`vtable` / `typeinfo` / `guard variable` / `thunk` の集約を確認できます。

### C++ 集約単位で差分を見る

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --demangle=on \
  --cpp-view \
  --group-by cpp-class
```

`--group-by` には次を指定できます。

- `symbol`
- `cpp-template-family`
- `cpp-class`
- `cpp-runtime-overhead`
- `cpp-lambda-group`

HTML と JSON では、C++ diff として template family / class / runtime overhead / lambda group の増減を表示します。class と template family の diff 行には、上位 symbol と why-linked 要約も併記されます。

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

### separate debug / split DWARF を使って source line を読む

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --dwarf=on \
  --source-lines lines \
  --debug-file-dir build/debug \
  --debug-trace
```

このときの debug artifact 解決順は次のとおりです。

1. main ELF に内蔵された debug section
2. `--debug-file-dir` で指定したディレクトリ
3. `.gnu_debuglink`
4. build-id による `.build-id/xx/yyyy.debug`
5. split DWARF sidecar (`.dwo` / `.dwp`)
6. `debuginfod`

補足:

- 同じ ELF を同一 process 内で繰り返し解析した場合、DWARF parse 結果は in-memory cache を再利用します
- `line = 0` や compiler-generated range は `unknown source` に寄せて表示します
- split DWARF sidecar が解決できれば、その `.dwo` / `.dwp` を使って attribution を継続します
- split DWARF marker があるのに使える sidecar が見つからない場合、`--dwarf=auto` は warning 付きで継続し、`--dwarf=on` は明確にエラーを返します
- `--debug-trace` を付けると、どの debug artifact をどの順番で探したかを標準出力で確認できます
- `--debuginfod=auto|on|off`、`--debuginfod-url`、`--debuginfod-cache-dir` は最後段の fallback 制御です。現行版では provenance 記録と graceful fallback までを扱い、実ネットワーク取得自体は未実装です

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
  --git-repo .
```

### 履歴一覧を表示する

```bash
fwmap history list --db history.db --limit 20
fwmap history list --db history.db --limit 20 --json
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

`ld.lld` の native text map を明示したい場合は `--map-format lld-native` を使います。既定は `auto` で、`VMA / LMA / Size / Out / In` ヘッダを見て自動判定します。

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

### 5.9.2 policy as code を使う

`--policy <path>` で TOML 形式の policy v2 を読み込めます。policy は外部 rule より広い範囲を扱い、profile ごとの budget、owner、waiver をまとめて管理します。

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf build/app-prev.elf \
  --prev-map build/app-prev.map \
  --policy tests/fixtures/sample_policy_v2.toml \
  --profile release \
  --policy-dump-effective \
  --report-json fwmap_policy.json \
  --sarif fwmap_policy.sarif
```

この設定でできること:

- region ごとの absolute budget を判定する
- source path / library ごとの delta budget を判定する
- C++ class / template family ごとの budget を判定する
- path / object / library / C++ 集約単位へ owner を付与する
- 期限付き waiver と期限切れ waiver を管理する

最小例:

```toml
version = 2
default_profile = "release"

[profiles.release.budgets.regions.FLASH]
max_bytes = 524288
warn_bytes = 500000

[profiles.release.budgets.paths."src/net/**"]
max_delta_bytes = 4096

[[owners]]
owner = "network-team"
[owners.match]
paths = ["src/net/**"]

[[waivers]]
rule = "budget.path.delta"
expires = "2026-12-31"
reason = "legacy migration in progress"
[waivers.match]
paths = ["src/legacy/**"]
```

補足:

- profile を省略した場合は `default_profile`、なければ `default`、それもなければ先頭 profile を使います
- active waiver は `policy.waived` に残り、通常の violation には出しません
- expired waiver は violation を止めず、追加で `POLICY_WAIVER_EXPIRED` を出します
- HTML / JSON / SARIF に policy 情報が反映されます

### 5.9.3 demangle の使い分け

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
  --git-repo .
```

これで 1 回分の解析結果を SQLite に保存できます。

`analyze` から直接 history を保存する場合:

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --save-history \
  --history-db history.db
```

Git リポジトリを明示する場合:

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --save-history \
  --history-db history.db \
  --git-repo .
```

Git メタデータを無効化する場合:

```bash
fwmap analyze \
  --elf build/app.elf \
  --map build/app.map \
  --save-history \
  --no-git
```

commit timeline を見る場合:

```bash
fwmap history commits --repo . --limit 50 --order ancestry
fwmap history commits --repo . --branch main --json
```

commit range の差分を見る場合:

```bash
fwmap history range main~20..main --repo . --include-changed-files
fwmap history range main...feature/foo --repo . --json
fwmap history regression --metric rom_total main~50..main --threshold +8192 --repo .
fwmap history regression --rule ram-budget-exceeded main~50..main --include-evidence --json
fwmap history regression --entity source:src/net/proto.cpp v1.2.0..HEAD --include-changed-files --html regression.html
```

`history commits` は Git の commit 順に沿って解析済み build を並べ、前回解析済み commit 比の ROM/RAM 差分を表示します。`history range` は `A..B` と `A...B` の両方に対応し、累積差分、worst commit、missing-analysis commit 数、changed files と source diff の交差を確認できます。`history regression` は metric / rule / entity の起点推定を行い、`last_good`、`first_observed_bad`、`first_bad_candidate`、confidence、reasoning、evidence を返します。

一覧確認:

```bash
fwmap history list --db history.db --limit 20
fwmap history list --db history.db --limit 20 --json
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
fwmap history trend --db history.db --metric object:build/main.o --last 20
fwmap history trend --db history.db --metric archive-member:libapp.a(startup.o) --last 20
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
- 上位 object の why linked 要約
- Git short hash / branch / describe / dirty flag / subject

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
| `--why-linked-top <n>` | 任意 | diff 上位項目へ why linked 説明を追加する件数 |
| `--sarif <path>` | 任意 | SARIF 2.1.0 出力先 |
| `--sarif-base-uri <uri>` | 任意 | SARIF の repo relative path 用 base URI |
| `--sarif-min-level <info|warn|error>` | 任意 | SARIF に含める最小 warning level |
| `--sarif-include-pass <true|false>` | 任意 | SARIF properties に pass metadata を含めるか |
| `--sarif-tool-name <name>` | 任意 | SARIF `tool.driver.name` の上書き |
| `--rules <path>` | 任意 | 外部 TOML ルール設定 |
| `--policy <path>` | 任意 | policy v2 TOML 設定 |
| `--profile <name>` | 任意 | 使用する policy profile 名 |
| `--policy-dump-effective` | 任意 | 選択された effective policy summary を表示 |
| `--demangle=auto|on|off` | 任意 | C++ symbol demangle 制御 |
| `--toolchain <auto|gnu|lld|iar|armcc|keil>` | 任意 | map parser family の自動判定または強制指定 |
| `--map-format <auto|gnu|lld-native>` | 任意 | map text format の自動判定または強制指定 |
| `--dwarf=auto|on|off` | 任意 | DWARF line table の使用有無 |
| `--symbol <name>` | 任意 | `explain` で symbol の why linked を表示 |
| `--object <name>` | 任意 | `explain` で object / archive member の why linked を表示 |
| `--section <name>` | 任意 | `explain` で section placement の理由を表示 |
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
| `history record --db <path> --elf <path>` | 履歴を 1 件保存。`--git-repo <path>` / `--no-git` を指定可能 |
| `history list --db <path> [--limit <n>] [--json]` | 保存済み履歴の一覧表示。Git 情報を含む JSON 出力にも対応 |
| `history show --db <path> --build <id>` | 特定 build の詳細表示 |
| `history trend --db <path> --metric <metric>` | 推移表示 |
| `history commits [--repo <path>]` | Git commit 順に解析済み build を一覧表示。`--json` / `--html` 対応 |
| `history range <A..B|A...B>` | commit range の累積差分と worst commit を表示。`--include-changed-files` / `--json` / `--html` 対応 |
| `history regression (--metric <key> \| --rule <id> \| --entity <key>) <A..B\|A...B>` | 回帰起点を推定。`--mode` `--threshold` `--jump-threshold` `--include-evidence` `--include-changed-files` `--json` `--html` 対応 |

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
- `git`
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

`git` には `repo_root`, `commit_hash`, `short_commit_hash`, `branch_name`, `detached_head`, `tag_names`, `commit_subject`, `author_name`, `author_email`, `commit_timestamp`, `describe`, `is_dirty` が入ります。Git が使えない場合や `--no-git` 指定時は `null` です。

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
14. Object Details
15. Archive Details
16. Diff
17. Footer

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
- Git short hash / branch / describe / dirty state

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
- object / archive の検索
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

### 8.10 Object Details / Archive Details

`Top Object Contributions` の次に、object / archive を why linked と合わせて見るための詳細表が表示されます。

確認できる内容:

- `Object Details`: object ごとの合計サイズ、section 数、why linked 要約、confidence、trend コマンド
- `Archive Details`: archive ごとの member 数、合計サイズ、archive pull 件数、whole-archive 推定の有無、trend コマンド

使い方の目安:

1. `Top Object Contributions` で増えている object を見つける
2. `Object Details` で why linked 要約と confidence を確認する
3. archive member 由来なら `Archive Details` で whole-archive の影響や pull evidence を確認する
4. 表示されている `history trend` コマンドをそのまま実行して推移を見る

### 8.11 Diff

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

- CLI: [src/cli/mod.rs](src/cli/mod.rs)
- ELF parser: [src/ingest/elf/mod.rs](src/ingest/elf/mod.rs)
- map parser: [src/ingest/map/mod.rs](src/ingest/map/mod.rs)
- linker script parser: [src/ingest/linker/mod.rs](src/ingest/linker/mod.rs)
- dwarf parser: [src/ingest/dwarf/mod.rs](src/ingest/dwarf/mod.rs)
- analyze: [src/core/analyze.rs](src/core/analyze.rs)
- rules: [src/core/rules.rs](src/core/rules.rs)
- rule config: [src/core/rule_config.rs](src/core/rule_config.rs)
- demangle: [src/core/demangle.rs](src/core/demangle.rs)
- history: [src/core/history.rs](src/core/history.rs)
- diff: [src/core/diff.rs](src/core/diff.rs)
- model: [src/core/model.rs](src/core/model.rs)
- render: [src/report/render.rs](src/report/render.rs)
- quality checks: [src/validation/quality.rs](src/validation/quality.rs)

## 13. 既知の制約

- ELF は現在 `SHT_SYMTAB` を中心に参照
- map は GNU ld と LLVM lld を正式対応
- `lld-native` は ELF 向け `ld.lld -Map` / `--print-map` の text format を対象にする
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
- line 0 や compiler-generated range は `unknown source` に寄せて表示する
- separate debug は `--debug-file-dir`、`.gnu_debuglink`、build-id、基本的な split DWARF sidecar 解決に対応している
- `debuginfod` は fallback metadata と trace までは扱うが、現行版では実ネットワーク取得は未実装

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
## 16. Rust Cargo Ingestion

`fwmap analyze` は Rust / Cargo 向けの artifact 解決にも対応しています。Cargo metadata と `cargo build --message-format=json` の出力があれば、解析対象の ELF を自動で特定しつつ Rust context を付与できます。

### 16.1 Cargo build 出力から解析する

```bash
cargo metadata --format-version=1 > build/cargo-metadata.json
cargo build --release --message-format=json > build/cargo-build.jsonl

fwmap analyze \
  --cargo-build-json build/cargo-build.jsonl \
  --cargo-metadata build/cargo-metadata.json \
  --cargo-package fwmap \
  --cargo-target-name fwmap \
  --cargo-target-kind bin \
  --cargo-target-triple x86_64-unknown-linux-gnu \
  --resolve-rust-artifact strict \
  --map target/release/fwmap.map \
  --report-json out/report.json \
  --out out/report.html
```

`--elf` を手で指定しなくても、Cargo の build JSON に十分な情報があれば実行ファイルや共有ライブラリを解決できます。

### 16.2 明示的な ELF に Rust context を付与する

```bash
fwmap analyze \
  --elf target/release/fwmap \
  --map target/release/fwmap.map \
  --cargo-metadata build/cargo-metadata.json \
  --cargo-package fwmap \
  --cargo-target-name fwmap
```

この場合は `--elf` が常に優先され、Cargo 入力は `rust_context` の付与だけに使われます。

### 16.3 曖昧さの扱い

- 複数の Cargo artifact が一致した場合、`fwmap` は自動で 1 つを選ばず、対処方法付きのエラーで停止します。
- 絞り込みには `--cargo-package`、`--cargo-target-name`、`--cargo-target-kind` を使います。
- `--elf` を指定した場合はそのパスを必ず使い、Cargo 入力は `rust_context` の補強にだけ使われます。
- metadata だけを与えた場合、`--allow-target-dir-fallback` を有効にしない限り target directory を推測探索しません。

### 16.4 History と JSON 出力

- Cargo 入力がある場合、`report.json` には `rust_context` が追加されます。
- `history.db` には Rust の package / target / profile / target triple を additive migration で保存します。
- `history list --json` と `history show` でも保存済みの Rust context を確認できます。

## 17. Rust View

`--view rust` を付けると、従来の ELF / map 解析結果を保ったまま、Rust 開発者が見たい単位でサイズを追えるようになります。

### 17.1 基本の使い方

```bash
fwmap analyze \
  --elf target/release/fwmap \
  --map target/release/fwmap.map \
  --cargo-metadata build/cargo-metadata.json \
  --cargo-build-json build/cargo-build.jsonl \
  --cargo-package fwmap \
  --cargo-target-name fwmap \
  --view rust \
  --report-json out/report.json \
  --out out/report.html
```

この表示では主に次を確認できます。

- package ごとのサイズ
- target ごとのサイズ
- crate ごとのサイズ
- dependency crate ごとのサイズ
- source file ごとのサイズ
- Rust symbol ごとのサイズ
- generic / closure / async / trait / function 系の grouped family

Rust metadata が不足している場合でも panic せず、Rust-attributed symbol が見つからない旨だけを表示して通常解析を続けます。

### 17.2 CLI と history

`--view rust` は `analyze` だけでなく history 系にも使えます。

```bash
fwmap history show --db history.db --build 12 --view rust
fwmap history range main~20..main --db history.db --repo . --view rust
```

Rust View を履歴へ保存した build では、`history show --view rust` で package / target / crate / dependency / source / family の要約を確認できます。

### 17.3 trend / regression で使える Rust キー

```bash
fwmap history trend --db history.db --metric rust-package:fwmap --last 20
fwmap history trend --db history.db --metric rust-target:fwmap --last 20
fwmap history trend --db history.db --metric rust-crate:serde --last 20
fwmap history trend --db history.db --metric rust-dependency:tokio --last 20
fwmap history trend --db history.db --metric rust-source:src/main.rs --last 20
fwmap history trend --db history.db --metric rust-family:fwmap::worker::poll --last 20

fwmap history regression \
  --db history.db \
  --repo . \
  main~50..main \
  --metric rust-dependency:tokio.size \
  --threshold +16384
```

regression では `.size` 付きの metric key を使います。trend では `rust-package:...` のようなキーだけを指定します。

### 17.4 HTML / JSON に追加される項目

HTML の `Rust View` セクションでは、次のような集計を表示します。

- Rust Total
- Top Package
- Top Dependency
- Top Generic
- Top Async
- Top Rust Packages
- Top Rust Targets
- Top Rust Crates
- Dependency Crates
- Rust Source Files
- Grouped Rust Families
- Largest Rust Symbols

差分がある場合は `Rust Diff` セクションに次を表示します。

- Rust Package Delta
- Rust Target Delta
- Rust Crate Delta
- Dependency Crate Delta
- Rust Family Delta
- Rust Symbol Delta

JSON には optional な `rust_view` と `rust_diff` が追加されます。Rust symbol が取れない build では `null` になる場合があります。

### 17.5 grouping の考え方

Rust family の grouping は完全な意味解析ではなく、再現性を優先した deterministic なルールでまとめています。

- generic family: `<...>` の具体型パラメータをたたんで同一 family に寄せる
- closure family: `{{closure}}` を含む関数をまとめる
- async family: `poll`, `Future`, `GenFuture`, `{{async}}` などをまとめる
- trait family: `<T as Trait>` 形式をまとめる
- function family: 上記に当てはまらない通常の Rust function / method をそのまま扱う

そのため proc-macro 展開や複雑な monomorphization を完全には識別しませんが、build 間の比較に使うキーとしては安定することを重視しています。


## デスクトップアプリ

`apps/fwmap-desktop/` には Tauri 2 + React + HeroUI ベースのデスクトップ UI があります。解析そのものは既存の `fwmap` Rust core を使い、ローカルでの調査、比較、履歴確認、共有までを GUI からまとめて扱えるようにしています。

### セットアップ

```bash
cd apps/fwmap-desktop
npm install
```

### 開発起動

```bash
cd apps/fwmap-desktop
npm run tauri dev
```

### フロントエンドのビルド

```bash
cd apps/fwmap-desktop
npm run build
```

### Tauri バックエンドの確認

```bash
cargo check --manifest-path apps/fwmap-desktop/src-tauri/Cargo.toml
```

### できること

- ELF / map / rule file / Git リポジトリのパスを選んで解析を開始できます。
- Tauri event で `job-created` / `job-progress` / `job-finished` / `job-failed` を受け取り、最近の run と結果を確認できます。
- 実際の解析履歴は既存の `history.db` に保存しつつ、デスクトップ側の SQLite には設定、recent runs、plugin state、investigation、recent packages を保存します。
- `Runs`、`Diff`、`History`、`Regression`、`Inspector`、`Investigations` を GUI から順に辿れます。
- `Dashboard` では ROM / RAM trend、warning pressure、region usage、top growth contributors、recent regressions を表示します。
- `Workspace` / `Project` で既定の ELF / map / rules / Git repo / export 先を使い回せます。
- Policy Editor でポリシーファイルを読み込み、検証し、保存できます。
- HTML / print-friendly HTML / JSON で snapshot export ができます。
- 調査ごとに baseline / target を固定し、evidence、note、timeline、verdict をまとめて管理できます。

### 画面ごとの役割

- `Dashboard`: 最新 build の要約、履歴トレンド、warning pressure、recent regressions を確認します。
- `Runs`: 最近の解析 run 一覧と run detail を確認します。
- `Diff`: 2 つの run を比較し、ROM / RAM / warnings に加えて section / object / source / symbol / Rust の差分を見ます。
- `History`: commit timeline、range diff、regression origin を Git-aware に辿ります。
- `Inspector`: region / section / source / function / symbol / Rust aggregate を treemap / icicle / table で深掘りします。
- `Investigations`: 調査ケースを一覧し、baseline / target、evidence、note、timeline、verdict、package export をまとめて管理します。
- `Settings`: workspace/project、policy、history.db、export 先などを設定します。
- `Plugins`: built-in plugin の一覧、capability、extension point、enabled state を確認します。
- `Packages`: investigation package の作成、recent package の再オープン、manifest の確認を行います。

### ダッシュボード関連 API

- `desktop_get_dashboard_summary`

### ワークスペース / ポリシー / エクスポート関連 API

- `desktop_list_projects`
- `desktop_create_project`
- `desktop_get_active_project`
- `desktop_set_active_project`
- `desktop_update_project`
- `desktop_delete_project`
- `desktop_load_policy`
- `desktop_validate_policy`
- `desktop_save_policy`
- `desktop_export_report`
- `desktop_list_recent_exports`

### Inspector 関連 API

- `desktop_get_inspector_summary`
- `desktop_get_inspector_breakdown`
- `desktop_get_inspector_hierarchy`
- `desktop_get_inspector_detail`
- `desktop_get_source_context`

### Investigation 関連 API

- `investigation_create`
- `investigation_list`
- `investigation_get`
- `investigation_update`
- `investigation_delete`
- `investigation_add_evidence`
- `investigation_remove_evidence`
- `investigation_add_note`
- `investigation_update_note`
- `investigation_list_timeline`
- `investigation_set_verdict`
- `investigation_export_package`

### Investigation Workflow

`Investigations` は、差分や回帰候補を単発で眺めるだけで終わらせず、1 件の調査として継続的にまとめるための画面です。

主な流れ:

- `Diff`、`History`、`Regression`、`Inspector` から気になる項目を evidence として pin する
- baseline / target を固定して、どの build / commit を比べている調査かを明確に保つ
- note を追加して、仮説、確認ポイント、レビュー向けメモを積み上げる
- timeline で evidence 追加や verdict 更新の流れを追う
- 最後に verdict を付けて、根本原因の見立てと次のアクションを残す

verdict の例:

- `code change`
- `compiler/codegen change`
- `linker layout change`
- `dependency update`
- `build/config change`
- `mixed`
- `unknown`

### プラグイン

デスクトップアプリには、manifest-driven な built-in plugin の基盤があります。現時点では外部 shared library を読み込む方式ではなく、安全性と挙動の明確さを優先した内蔵プラグイン方式です。

現状の extension point:

- `analyzer.summary`
- `report.package-section`
- `visualization.adapter`

現状の built-in plugin:

- `size-posture-analyzer`
  - run / diff / history 系の情報から短い注釈を作ります。
- `package-provenance-exporter`
  - investigation package の provenance と inclusion summary を補足します。
- `timeline-signal-adapter`
  - timeline / range 結果をダッシュボードや package viewer 向けの signal block に整形します。

利用できる API:

- `desktop_list_extension_points`
- `desktop_list_plugins`
- `desktop_get_plugin_detail`
- `desktop_set_plugin_enabled`
- `desktop_run_plugin`

### Investigation Package

共有用の調査結果は `.fwpkg` ディレクトリ bundle として保存します。現在は zip ではなく、`manifest.json` と関連 JSON を並べた読みやすい bundle です。`Investigations` から書き出す場合は、調査タイトル、baseline / target、evidence、note、timeline、verdict をまとめて持ち出せます。

主な用途:

- issue 調査やレビュー依頼に、現在見ている run / diff / history / regression / inspector の要点を渡す
- release review 用に、比較結果と回帰候補の evidence をまとめる
- あとで reopen して、当時の調査コンテキストを再確認する

manifest に含める情報の例:

- package version / schema version / created at / fwmap version
- project metadata / git metadata / source context
- related run ids / related commit refs
- included files / omitted files
- export provenance
- plugin results

含められる内容:

- dashboard summary
- run detail
- diff result
- commit timeline
- range diff
- regression result
- investigation summary / evidence / notes / timeline / verdict
- inspector summary / detail / source context
- charts snapshot
- policy snapshot

利用できる API:

- `desktop_create_investigation_package`
- `desktop_open_investigation_package`
- `desktop_get_investigation_package_summary`
- `desktop_list_recent_packages`
- `desktop_export_package`

### 使い方の流れ

1. `Settings` で `history.db`、既定の ELF / map / rules、Git リポジトリを設定します。
2. `Start Analysis` から解析を実行し、`Runs` で結果を確認します。
3. 必要に応じて `Diff`、`History`、`Regression`、`Inspector` へ進みます。
4. 気になる差分は `Investigations` で 1 件の調査にまとめ、evidence の pin、note 追加、verdict 記録を進めます。
5. 共有したいタイミングで `Investigations` または `Packages` から package name、destination path、含める情報を設定して bundle を作ります。
6. 作成済み package は `Packages` で reopening し、manifest、included / omitted items、plugin results を確認できます。

### 現在の制約

- `cancel job` は UI 上のプレースホルダで、実際の中断処理はまだ未実装です。
- plugin system は built-in 前提で、外部 shared library plugin はまだ扱いません。
- investigation package は directory bundle で、圧縮 zip 形式や署名付き package ではありません。
- investigation 自体の保存先は desktop ローカル DB で、CLI の `history.db` とは分けて管理します。
- package viewer は manifest と保存済み JSON の再表示が中心で、完全な artifact browser ではありません。
- regression は lightweight な summary を優先しており、重い可視化や高度な interactive compare session は未実装です。
- GUI は CLI を置き換えるものではなく、既存 core / report / history の表示と操作をデスクトップ向けにまとめたものです。
