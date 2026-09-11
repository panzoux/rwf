# rwf 強化ロードマップ

**作成**: 2026-05-23  
**目標**: rwf を twf（C#プロトタイプ）と同等以上の機能・安定性に引き上げる  
**rwf の強み**: CJK文字表示、クロスプラットフォーム、メモリ効率、型安全性  
**現状完成度**: 約72%（コアロジック充実、UIダイアログ・ビューア系が主な不足）

---

## 凡例

- `[ ]` 未着手
- `[~]` 部分実装（UI要改善 or バックエンドのみ）
- `[x]` 完了
- `[-]` 不採用（設計判断により実装しない。理由は詳細欄／詳細ファイル参照）

---

## Phase 1 — UIダイアログ穴埋め（バックエンド済み、UI未実装）

> バックエンドロジックが既存。UIラッパーを実装して基本操作を完結させる。

| # | 機能 | 状態 | テスト方針 |
|---|------|------|-----------|
| 1.1 | **ソートダイアログ** + 昇降順トグル | `[x]` | ダイアログレンダリング + ソート結果の単体テスト |
| 1.2 | **ファイルマスクダイアログ** | `[x]` | マスク適用前後のファイルリスト検証 |
| 1.3 | **ワイルドカードマーキングダイアログ** | `[x]` | MarkingModelとの統合テスト |
| 1.4 | **シンプルリネームダイアログ** | `[x]` | リネーム前後のファイル存在確認 (tempfile) |
| 1.4.1 | **外部コマンド完了後のペインリフレッシュ** | `[x]` | `state.rs` ExecuteCustomFunction 完了時にアクティブペインへ ReadDirectory 投入 |
| 1.4.2 | **ポーリング間隔の設定フィールド追加** | `[x]` | `config.rs` に `polling_interval_ms: u32`（デフォルト 1000）。Layer 2 ポーリング実装（Phase 7）に先行して設定スキーマを確定 |
| 1.5 | **衝突解決ダイアログ**（コピー/移動時） | `[x]` | 上書き/スキップ/名前変更/キャンセル各パスのテスト |
| 1.6 | **ヒストリダイアログ** | `[x]` | 履歴リストの表示・選択・ナビゲーション |
| 1.7 | **ドライブ変更ダイアログ** | `[x]` | OS ドライブ列挙バックエンド + 選択リスト UI（Windowsドライブレター・Unix マウントポイント対応） |
| 1.8 | **ファイル情報ダイアログ** | `[x]` | `Dialog::file_info()` コンストラクタ済み、UIレンダリングのみ（名前・パス・サイズ・日時・パーミッション） |
| 1.9 | **パターンリネームダイアログ** | `[x]` | パターン入力テキストボックス + ライブプレビュー一覧（`PatternRename { pattern, preview }` モデルあり） |
| 1.10 | **バージョン情報（タスクペイン出力）** | `[x]` | 起動時＋バックティク（`` ` ``）でタスクペインへシステム情報出力（ダイアログ不使用） |
| 1.11 | **ヘルプダイアログ** | `[x]` | `HelpContent` 実装済み、スクロール対応 UI・言語ローテーション・9テスト |
| 1.12 | **登録フォルダ選択ダイアログ** | `[x]` | `RegisteredFolderManager` 完全実装済み（load/save・env変数展開・フィルタ）、UI選択リスト + インクリメンタルフィルタ・11テスト |

**推定規模**: 各500〜800行  
**リスク**: 低（既存 DialogStack と同じパターン）

---

## Phase 2 — ジャンプ・ナビゲーション

| # | 機能 | 状態 | 詳細 | テスト方針 |
|---|------|------|------|-----------|
| 2.1 | **Jump to Path ダイアログ** | `[x]` | 複数キーワードAND絞り込み、非同期補完 | パス補完・AND検索のユニットテスト |
| 2.2 | **Jump to File ダイアログ** | `[x]` | 再帰検索、ignoreリスト対応 | 実FS上の統合テスト (tempfile) |

**推定規模**: 各800〜1200行  
**リスク**: 中（再帰検索の非同期キャンセル処理）

---

## Phase 3 — ジョブ管理UI

> 詳細仕様は [plan_job_dialog.md](plan_job_dialog.md) を参照（推定60〜86時間）。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 3.1 | **タスクパネル** | `[x]` | 折り畳み/展開、ログ、スピナーアニメーション |
| 3.2 | **ジョブマネージャダイアログ** | `[x]` | 進捗表示、キャンセル操作、表示内容の洗練 |
| 3.3 | **タブのビジーインジケーター** | `[x]` | アクティブジョブ時のスピナー（TabBarView連携） |

**テスト**: ジョブ状態遷移の単体テスト + UIレンダリングのスナップショットテスト

---

## Phase 4 — テキスト/バイナリビューア

> モデル層 ([`model/viewer.rs`](../rwf-lib/src/model/viewer.rs)) は実装済み。  
> `ViewerMode::Text` と `ViewerMode::Hex` の両方がある（Hexも自前実装済み）。  
> TWFも同様に自前実装。不足はTUIレンダリング層とエンコーディング実装。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 4.1 | **テキストビューア TUI ウィジェット** | `[x]` | スクロール、行番号、検索ハイライト |
| 4.2 | **Hex/バイナリビューア TUI ウィジェット** | `[x]` | `get_hex_bytes_vec()` + `hex_row_spans()`（rwf-bin）でオフセット・ASCII表示（旧 `get_hex_line()` は 7.3b で未使用と判明し削除） |
| 4.3 | **大容量ファイル対応** | `[x]` | `memmap2` によるメモリマップ + `LineIndex` バックグラウンドインデックス（ファイル全体をRAMに乗せない） |
| 4.4 | **エンコーディング実装補完** | `[x]` | Shift-JIS/EUC-JP を `encoding_rs` クレートで完全実装済み |
| 4.5 | **エンコーディング自動検出** | `[x]` | BOM検出 + 日本語統計的検出を `TextEncoding::detect()` として実装済み |
| 4.6 | **サイドバイサイドビューアモード** | `[x]` | `v`=フルスクリーン、`V`=サイドバイサイド、Tab/Shift+Tab フォーカス移動 ([詳細](4.6.side-by-side_viewer_mode.md)) |

**追加クレート候補**:
- `encoding_rs` — Shift-JIS/EUC-JP等のデコード（Mozilla製、クロスプラットフォーム）

**テスト**: エンコーディング検出ユニットテスト、大容量ファイルのメモリ使用量テスト、Hexレンダリング検証

---

## Phase 5 — アーカイブ拡張

> 現状: `zip`クレートのみ。TWFは外部`7z.exe`（Windowsのみ）を使用。  
> rwfはクレートベースでクロスプラットフォーム対応を優先する。

| # | 機能 | 状態 | クレート/方針 |
|---|------|------|-------------|
| 5.1 | **7z サポート** | `[x]` | `sevenz-rust`（純Rust、win/mac/linux対応）`SevenZArchiveHandler` + `MultiFormatArchiveHandler` 実装済み、9テスト合格 |
| 5.2 | **TAR/TGZ サポート** | `[x]` | `tar` + `flate2` クレート。`TarArchiveHandler`実装済み（.tar/.tgz/.tar.gz）、10テスト合格 |
| 5.3 | **RAR サポート** | `[ ]` | `.rar` は認識済み（graceful error）。将来: `libarchive` クレート経由で実装予定（→ 5.6 参照） |
| 5.4 | **ISO サポート** | `[x]` | `iso9660` クレート（純Rust）でブラウズ・展開実装済み。作成不可（read-only） |
| 5.5 | **LZH サポート** | `[ ]` | `.lzh/.lha` は認識済み（graceful error）。将来: `libarchive` クレート経由で実装予定（→ 5.6 参照） |
| 5.6 | **libarchive 統合（RAR・LZH 他）** | `[ ]` | `compress-tools` クレート（`libarchive` ラッパー）で RAR/LZH/CAB 等を一括対応。libarchive がインストール済みの環境で有効化。将来の機能強化フェーズで実装 |

**テスト**: 実アーカイブファイルを使った統合テスト（各形式で作成→展開→内容確認）

---

## Phase 6 — twfパリティ完結（高度機能）

> Phase 6完了でtwfとの完全パリティ達成。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 6.1 | **設定システム・ペイン更新機構整備** | `[~]` | Layer 1更新機構(外部コマンド後の自動リフレッシュ)実装済み。ConfigManagerに extension_associations.json / custom_functions.json / context_menu.json パス追加。colors.json分離は未実装（後フェーズ） |
| 6.2 | **ファイルタイプ関連付け** | `[x]` | `ExtensionAssociation`構造体、`extension_associations.json`読み込み、Enter時の拡張子マッチ→外部コマンド実行。マクロ展開対応。AppState起動時ロード・Shift+Zでリロード。 |
| 6.3 | **カスタム関数システム** | `[x]` | `custom_functions.json`読み込み、Shift+T でカスタム関数選択ダイアログ表示・実行。インクリメンタルフィルタ、マクロ展開対応、PipeToAction対応。AppState起動時ロード・Shift+Zでリロード。 |
| 6.4 | **コンテキストメニューシステム** | `[x]` | `\`キー でコンテキストメニュー表示。デフォルト組み込みアクション(View/Copy/Move/Rename/Delete)+セパレータ対応。カスタム関数呼び出し対応(`ContextMenuAction::CustomFunction`)。上下ナビ（セパレータスキップ）実装。 |
| 6.5 | **カスタム関数メニューダイアログ** | `[x]` | `menu_xxx.json` 対応。メニュー型関数（`Menu` フィールド）を選択時に専用メニューダイアログを表示。`Action` フィールドでカスタム関数名またはビルトインアクション名を解決・実行。セパレータスキップ、文字キージャンプ対応。詳細: `plan/phase-6-6-custom-function-menus.md` |
| 6.6 | **ビューア大容量ファイルエンジン（LargeFileEngine 方式）** | `[x]` | mmap を廃止し `FileBytes::Seekable(SeekableFile)` へ移行。`File + Seek + Read` でページフォルト遅延を根絶。Hex検索もチャンク読みで対応。InMemoryしきい値は `viewer_large_file_threshold_mb`（デフォルト100MB）で設定可能。memmap2 依存を完全削除。 |
| 6.7 | **ヘルプ強化（実キーバインドビューア）** | `[x]` | `?`/F1 オンラインヘルプは修正済み（ハードコード表示）。設定変更を即反映する動的キーバインドビューアは未実装。Phase 6 機能セット確定後に対応 |

---

## 設計決定事項 — ペイン更新機構

### 更新機構の2層モデル

ファイラーとして「本来あるべきファイルが見えない」状況は最大級のストレス。
効率より表示正確性を優先し、以下の2層で対応する。

#### Layer 1 — 操作起因の即時リフレッシュ

各操作完了後に、影響範囲に該当するペインへ `ReadDirectory` を投入する。

| 操作の種類 | アプリが知っていること | 更新方式 |
|-----------|-------------------|---------|
| Rename（単体） | from→to が完全既知 | インメモリ更新（フラッシュなし） |
| Copy / Move / Delete / Mkdir | 変化したディレクトリが既知 | ReadDirectory（同パスの全ペイン） |
| PatternRename | 変化したディレクトリが既知 | ReadDirectory（アクティブペイン） |
| ExtractArchive / CreateArchive | dest ディレクトリが既知 | ReadDirectory（同パスの全ペイン） |
| **外部コマンド（カスタム関数）** | **不明** | **アクティブペインを ReadDirectory** |

**外部コマンドの扱いに関する設計決定（2026-05-24）**:  
`refresh_after` のような影響範囲宣言をユーザーに求める案も検討したが採用しない。
定義漏れ・誤設定のリスクがあり、ユーザビリティを低下させる。
外部コマンドは「OS/外部プロセスによる変化」と同等に扱い、完了後に
アクティブペインを無条件リフレッシュする。効率は若干犠牲になるが、
表示正確性・公平性・設定の単純さを優先する。

#### Layer 2 — バックグラウンドポーリング（外部変化の追跡）

Layer 1 が捉えられない外部プロセス・他アプリによる変化を補完する。

- 方式: 表示中エントリのメタデータ（サイズ・更新日時）をタイマーで定期チェック（twf の `PerformSmartRefresh` 相当）
- 差分があれば ReadDirectory を投入
- 対象: ローカルドライブ、ネットワークドライブ、SDカード、クラウド同期ドライブ（Box・OneDrive等）
- twf での実績: 比較的遅いネットワーク/クラウドドライブでも安定動作を確認

**FSWatcher（notify クレート）は採用しない理由**:  
ネットワークドライブ・仮想FSでのイベント欠落が実用上の問題になりやすく、
ポーリングより信頼性が低い場面がある。ポーリングで十分な精度が得られる。

#### 例外 — アーカイブ仮想FS

- FSWatcher の対象外（実ファイルシステムではない）
- Layer 2 のポーリング対象外
- アーカイブ操作完了後の Layer 1 リフレッシュのみ適用

#### 実装優先度

| 機構 | フェーズ | 状態 |
|------|---------|------|
| Layer 1: 内部ジョブ後リフレッシュ | Phase 1〜5 | 既存 ✅ |
| Layer 1: 外部コマンド後アクティブペインリフレッシュ | Phase 1.4.1 | 実装済み ✅ |
| Layer 2: バックグラウンドポーリング | Phase 7.5 | `[ ]` |

---

## Phase M — 品質整備フェーズ（**完了・凍結解除済み**。詳細: [quality_overhaul.md](quality_overhaul.md)）

> **🔓 機能開発凍結解除宣言（2026-07-13）**: M1〜M7 全タスク完了。検証一式（fmt / clippy --all-targets
> -D warnings / `cargo test -p rwf` 156 / `cargo test -p rwf --no-run` / `cargo test -p rwf-lib` 1044、
> 3042.45s）全緑。本計画全体を通じた挙動変更は M7 archive.rs の 1 件のみ（詳細は quality_overhaul.md の
> 「Phase M 完了サマリ」参照）。Phase 7 残タスクおよび Phase 8+ の機能開発に着手可能。

> **背景**: 複数 AI（kilo→antigravity→qwen→gemini→claude）を渡り歩いた開発による ad-hoc コード蓄積への対処。
> 本来の目的は一回きりの掃除ではなく「AI 主導開発でも品質が劣化しない仕組み」の構築。
> Phase 7 の残タスクに着手する前に本フェーズを完了させ、後続開発の負荷を下げた状態で Phase 7 以降を実施する
> という方針のもと、M 完了までは新機能実装を凍結していた（全タスクは挙動保存、M7 の archive.rs TODO 修正のみ例外）。
> 各タスク完了条件: fmt / clippy -D warnings / 全テスト緑 + コミット。

| # | タスク | 状態 | 概要 |
|---|---|---|---|
| M1 | ガードレール導入 | `[x]` | **完了（2026-07-05）** rustfmt.toml + cargo fmt 一括適用（独立コミット 2f34739, blame-ignore 登録）/ clippy.toml（allow-unwrap-in-tests）/ workspace lints（unsafe_code deny + volume_info.rs のみ SAFETY 付き allow, unwrap_used deny + 9 モジュール allow ratchet）/ CI に fmt --check 追加 |
| M2 | 共有部品・ドキュメント基盤 | `[x]` | **完了（2026-07-05）** rwf-lib test_utils 新設 + 40 テストファイル fixture 移行（テスト件数 1043 不変。カスタム config・実 FS セットアップ持ち約 10 ファイルは意図的に未移行）/ ui/dialog/common.rs（スタイル定数 + titled_block、frame.rs 適用）+ ConflictInputHarness / ルート CLAUDE.md / ARCHITECTURE.md / TESTING.md / recipes ドラフト / stale 参照修正（two-pane-fm→rwf） |
| M3 | dialog/mod.rs 分割 | `[x]` | **完了（2026-07-05）** insta スナップショット安全網（全 29 バリアント × 2 サイズ = 94 テスト/188 snap、決定性 3 回検証）→ ダイアログ単位に 17 ファイルへ move-only 分割（mod.rs 5,409→2,024 行。残りは render_dialog/handle_dialog_input の dispatch — 腕本体の関数化は M4 の struct 化後が合理的なため M4 へ）+ common.rs 定数 81 箇所適用（snap 差分ゼロ）+ conflict テストを ConflictInputHarness へ移行・file_conflict.rs へ同居 |
| M4 | model/dialog.rs 分割 | `[x]` | **完了（2026-07-07）** 全29バリアントを struct 化（enum は維持。`DialogContent::Foo(FooDialog)`）/ `DialogUiState`（cursor_pos/scroll_pos/focused_field）を FileMask・WildcardMark・SimpleRename に導入 / `handle_dialog_input` の腕本体を各ダイアログファイルの `handle_input()` へ移動（rwf-bin/ui/dialog/mod.rs: 2,145→1,045 行、`handle_dialog_input` 自体は 1,322→222 行。残る腕はクロスカッティングな dispatch ロジックのみ）/ DIALOG_DESIGN_SPEC.md・add-a-dialog.md 更新 / unwrap allow スコープ確認（11箇所すべて `expand_env_vars` にあり、struct化ファイルには0件）。詳細は `plan/M4_handoff.md` 参照 |
| M5 | state.rs 分割 | `[x]` | **完了（2026-07-07）** state.rs(4,741行)を `state/` ディレクトリへ move-only 分割:実測 10 個の `handle_*_transition` を `state/handlers/{navigation,tab,marking,job,job_management,ui,view,search,viewer,advanced}.rs` へ(dialog 系は ui.rs 内に同居のまま、分離は判断コスト増のため見送り)/ 共有ヘルパは `state/helpers.rs`(editor_job のみ該当、他は実測で単一所有と判明)/ AppState 本体・unwrap 4箇所は不分割(mod.rs 残留)/ `docs/ARCHITECTURE.md` にフィールド所有権マップ追記 / `cargo test -p rwf-lib -- --list` 件数 1043 不変・フルテスト 1043 passed 確認。詳細は `plan/M5_handoff.md` 参照 |
| M6 | unwrap/clone 監査 | `[x]` | **完了（2026-07-12）** 非テスト unwrap 35 箇所・9 モジュール全て分類・処置(infallible→expect 24 箇所 / lock poisoning→expect 11 箇所 / エラー伝播への変更 0 箇所)、`#![allow(clippy::unwrap_used)]` 全撤去。clone 799 のうち FileEntry 系・ホットパス上位候補を Explore×haiku 2 体並列で調査 → 7 箇所を借用化で修正(marking ハンドラ 4 / search フィルタ 1 / pattern_rename 2)、6 系統はアーキテクチャ変更が必要なため churn 回避方針により見送り(理由は M6_handoff.md 参照)。検証一式(fmt/clippy/rwf 145/rwf-lib 1043)全緑。詳細は `plan/M6_handoff.md` 参照 |
| M7 | 仕上げ | `[x]` | **完了（2026-07-13）** add-a-dialog.md / add-a-transition.md を最終構造で確定（SortDialog で手順検証済み）/ backend・job・model に rustdoc 約50箇所追加（haiku×3並列 → sonnet レビューで1件の誤解を招く記述を修正）/ archive.rs の ZIP タイムスタンプ TODO 修正（**本計画唯一の挙動変更**、テスト追加）/ rwf-bin UI 未テスト4ファイル（panes/task_panel/viewer/tab_bar）へ TestBackend スモーク+スナップショット 11 件追加 / ルート `*_SUMMARY.md`/`BUGFIX_*.md` 11 件を `docs/history/` へ整理 / `#![warn(missing_docs)]` 導入は見送り（`model/dialog/` 約262項目・`rwf-bin/src/ui/` 約65項目が未着手のため、Phase 8+ 送り）/ **凍結解除宣言**。詳細は `plan/M7_handoff.md` 参照 |

---

## Phase 7 — twf超え（rwf独自の強化）

> CJK表示はすでに rwf の強み。さらに差別化できる機能。
> **着手条件: Phase M 完了 —満たされた（2026-07-13）。着手可能。**

### 推奨実装優先度（2026-07-05 番号再割当・状態更新）

> **番号再割当について（履歴）**:
> - 2026-07-02: 旧表の 7.1(Undo)/7.2(Leap) が plan/ 配下の実ファイル名
>   （`7.6.transactional_rollback.md`・`7.8.leap_navigation.md`）およびコミット履歴（`feat(7.8)` = Leap）と
>   不一致だったため、ファイル名側を正として再割当。7.1・7.2 は欠番となった。
> - 2026-07-05: 欠番だった 7.1 に **Leap（完了済み、旧 7.8）** を、7.2 に **コマンドパレット（旧 Phase 8.7 から昇格）** を割当。
>   7.8 は欠番。**詳細ファイル名 `7.8.leap_navigation.md` とコミット履歴 `feat(7.8)` は歴史的経緯としてそのまま**（リネームしない）。

> **表の行順は実装優先順（上ほど先）**。番号（#列）は歴史的経緯で振られた識別子であり、
> 詳細ファイル名（`7.4.xxx.md` 等）・コミット履歴（`feat(7.x)`）と対応するため固定。
> 実行順の一次情報はこの表そのもの（旧「Phase 7 実装順序」節は本表に統合・廃止）。

| # | 機能 | 状態 | 詳細 | 優先度 | 工期 |
|---|------|------|------|--------|------|
| 7.1 | **Leap ナビゲーション（高速フィルタ移動）** | `[x]` | **実装完了（2026-06〜07、705a392〜c8ff3e4、旧 7.8）**。F3 で Leap モード起動、AND セグメント + prefix/substring/Migemo union フィルタ、LEAP バー + スピナー、デフォルトキーバインド配線・キー衝突解消済み。詳細は [7.8.leap_navigation.md](7.8.leap_navigation.md) 参照 | ⭐⭐⭐⭐⭐ | 完了 |
| 7.3 | **スマート・ファイルオープナー（Rifle + コンテンツ判定）** | `[x]` | **実装完了（2026-07、234758f〜351b45b）+ 7.3b 拡張（10d802d まで）**。Phase 6.2 (ExtensionAssociations) の発展形 + 旧 8.7（マジックバイト判定）を統合。Open With ピッカー（単一/バッチ）、マジックバイトによる拡張子/実体不一致の警告、拡張子未登録ファイルのコンテンツ判定フォールバック、File Information ダイアログのオンデマンド判定表示。**7.3b（実機ドッグフーディング後の拡張）**: `extension_associations.json` に `FileType` フィールドを追加し検出タイプ優先・拡張子フォールバックの解決順に変更（旧: 拡張子のみ）、コンテキストメニュー/ピッカータイトルへの検出タイプ表示、File Info でのヘッダーバイト実表示（hex/text 切替、`t` キー）。詳細は [7.3.smart_file_opener.md](7.3.smart_file_opener.md) 参照 | ⭐⭐⭐⭐⭐ | 完了 |
| 7.11 | **属性/タイムスタンプ変更** | `[x]` | **実装完了（2026-07-30）**。ファイル属性（Windows: ReadOnly/Hidden/System/Archive、Unix: パーミッション、8進数直接入力+rwx連動表示）とタイムスタンプ（modified/accessed、Windowsのみcreatedも読み取り専用表示）の編集ダイアログ（`Ctrl+a`）。マーク複数選択の一括編集・混在状態表示に対応。touchはこのダイアログの `Now` キーへ統合。Windows属性設定は`volume_info.rs`（唯一のunsafe許可モジュール）へ`SetFileAttributesW`呼び出しを追加、タイムスタンプ設定は新規`filetime`クレート依存で対応。7.6 Undo対象表（Attribute Change / Timestamp Change）の前提操作。詳細は [7.6.attribute_timestamp_edit.md](7.6.attribute_timestamp_edit.md) 参照 | ⭐⭐⭐ | 完了 |
| 7.12 | **Create Link / Create File** | `[x]` | **実装完了（2026-07-30）**。シンボリックリンク/ハードリンク/(Windows)ジャンクション作成ダイアログ（`;`。当初`Ctrl+l`を予定していたがターミナルに予約されて使用不可と判明し変更）と空ファイル作成ダイアログ（`N`）。Create File はマクロ+外部コマンドでも代替可能だが、Job化することで 7.6 の Undo/Operation Report 追跡対象になれる点が内蔵化の主な価値。Junction作成は`mklink /J`シェルアウト（`/D`でAutoRun無効化— Clink等のシェル拡張によるexit code汚染を回避）。targetはダイアログを開いた時点のカーソル位置アイテムに固定・非入力、dest_dirは対向ペインのカレントディレクトリに固定・非入力(設計通り)。7.6 Undo対象表（Create Link / Create File）の前提操作は全て解消。詳細は [7.6.create_link_file.md](7.6.create_link_file.md) 参照 | ⭐⭐⭐ | 完了 |
| 7.15 | **デバッグレポート機能（診断セッション）** | `[x]` | **Stage 1〜6 実装完了（2026-08-11、worktree `worktree-phase-7.15-diagnostics`、main へ未マージ）**。`F12` 診断セッション開始/終了、`F11` スナップショット（NormalMode/ViewerMode/LeapMode 全てに割当）、`● DIAG mm:ss` インジケータ、終了時レポート入力ダイアログ、`RWF_DIAGNOSTICS=1` 環境変数トリガ、`Diagnostics` 設定セクション。バンドル = `events.jsonl`/`logs.jsonl`/`config_effective.json`/`metadata.json`/`snapshots/NNN-*.{txt,json}`/`report.txt`。観測点は `update_state()`・`JobManager::start_job()`・`handle_key_event()`・`render()`・main loop の Wake の5箇所。**実装中に判明した設計前提の誤り5件**（`Backend::buffer()` は `TestBackend` 専用で実バックエンドには無い / 全角文字の継続セルは空文字ではなく**スペース**のため表示幅で歩進が必要 / `AppConfig` の `key_bindings` は `#[serde(skip)]` でシリアライズされない / `DialogStack` に `iter()` は無い / `test_utils` は `#[cfg(test)]` gate で統合テストから使えない）はすべてテストが検出し、plan 側に日付付きで訂正記録済み。Ring buffer は v1 スコープ外。**7.6 と並列開発し main へ先行マージ**（7.6 が自身のデバッグに使えるようにするため）。詳細は [7.15.diagnostic_report.md](7.15.diagnostic_report.md) 参照 | ⭐⭐⭐⭐ | 未見積 |
| 7.6 | **Undo/Redo（トランザクション・ロールバック）** | `[x]` | **実装完了（2026-08-17）**。Copy/Move/Rename/Delete/Mkdir/CreateFile/CreateLink/CreateArchive を Operation Record 化し、既存の ChangeAttributes/ChangeTimestamps/MoveToTrash/RestoreFromTrash/ExtractArchive も Operation Report へ翻訳して統合。`Alt+o` で開く Operation Report ダイアログ（一覧+詳細ペイン、行選択）から Undo/Redo を実行——両方向とも単一の `ExecuteReversal` 機構が駆動し、実行前に pre-flight 検証を通す。唯一真に破壊的なケース（Delete の取り消し＝復元）は `recreate` 情報を持つ `Delete` バリアントとして自己連鎖する設計とし、Undo→Redo→Undo…を無制限に繰り返せる。実装過程で見つかった MoveToTrash ペイン再描画・EmptyTrash 確認ダイアログ表示重複の既存バグ（7.6 由来ではない）は**未トラッキング**（分離チケット未作成 — plan/・git log 全体・docs/superpowers/plans/ いずれにも該当なし。2026-08-XX 時点で要確認・要チケット化）。詳細は [7.6.operation_report_ui.md](7.6.operation_report_ui.md)（UI/UX）・[7.6.transactional_rollback.md](7.6.transactional_rollback.md)（力学）参照；後続の follow-up タスクで Operation Report ダイアログの履歴ブラウズを左右キーから **Shift+↑/↓**（`d8b4afc`/`ba1f595` で二段スタック式サイドバーへ再設計）に変更し（設定可能な `history_size` に従い直近N件まで）、実際の Undo/Redo トリガーは最新レポートのみに制限（`916a80d` で入力層 + `process_dialog_confirmation` の二重ガード。標準的な LIFO undo スタック方式——主要エディタと同じモデル） | ⭐⭐⭐⭐⭐ | 完了 |
| 7.2 | **コマンドパレット** | `[ ]` | 旧 Phase 8.7 から昇格。ヘルプビューア（`?`）で検索して `Enter` でアクションを直接実行。VS Code の `Ctrl+Shift+P` と同じ体験。ヘルプはすでに検索ボックスとフィルタ済みリストを持っており、不足しているのは「ハイライト中のアクションをディスパッチして閉じる」`Enter` キーの処理のみ。コンテキスト（NormalMode / ViewerMode）でアクション絞り込みが必要。**2026-07-18: 意図的に後回し** — コンテキスト絞り込み・他機能との自然な統合（Open With ピッカー等）を含めた設計をまだ詰めていないため、7.3/7.6 の後に着手 | ⭐⭐⭐ | 小規模 |
| 7.4 | **バックグラウンド・ディレクトリサイズ計算** | `[ ]` | **Shift+S** で再帰的ディレクトリサイズを非同期計算。エントリごとにサイズを段階的に埋める。スピナー + Task pane ログ。詳細は [7.4.calculate_directory_size.md](7.4.calculate_directory_size.md) 参照 | ⭐⭐⭐⭐ | 2.5週間 |
| 7.5 | **バックグラウンドポーリング（Layer 2）** | `[ ]` | 可視エントリのメタデータ定期チェック（twf PerformSmartRefresh 相当）。間隔は config `polling_interval_ms`（1.4.2 で追加済み） | ⭐⭐⭐ | 2週間 |
| 7.7 | **スマート・トラッシュ（ゴミ箱）管理** | `[x]` | Windows/macOS/Linux 各OS標準への対応。削除ではなくゴミ箱へ移動、復元サポート。**実装完了（2026-08-05、f3fe572〜4415701）**（move/restore/empty/browse-restore UI、確認ダイアログ汎用化、件数/サイズ表示）。**Task 17（自動削除）は 7.13 へスコープアウト**（下記）。Task 18（7.6 側の `InverseJobSpec` 配線確認）は 7.7 自体の実装物ではなく 7.6 のスコープに属するため、7.7 としてはこれで完了。詳細は [7.7.smart_trash.md](7.7.smart_trash.md) 参照 | ⭐⭐⭐ | 完了 |
| 7.9 | **シンタックスハイライト（ビューア）** | `[-]` | **不採用（2026-08-29）**。`syntect` クレートによるコードハイライト（旧7.6）を検討し、**実装しないと決定**。理由は**設計非適合**——rwf のビューアはランダムアクセス（`render_text_content` はビューポート分の行だけをデコードし、100MB 超は `SeekableFile::read_bytes` で seek+read。Phase 6.6/6.7 で mmap を廃してまで確立した「大きなファイルが即座に開く」性質）だが、syntect のパーサは**逐次かつステートフル**で、N 行目の色を正しく出すには 0..N-1 行目をパース済みである必要がある（複数行文字列・ブロックコメント）。埋めるには N 行ごとの `ParseState` チェックポイント機構＋破棄ポリシー＋ `Job` 化が必須で、見積 2 週間の大半がここに消え、ビューアのホットパスに恒久的な複雑度を残す。ハイライト付きでコードを読む用途は**外部ツールへ委譲**（6.2 `extension_associations.json` / 7.3 スマート・ファイルオープナー）。**サイズは判断理由ではない**（実測: `default-fancy` で +672 KB / +9.5%、7,269,888→7,957,504 B。構文定義75・テーマ7は `include_bytes!` 埋め込みで**外部ファイル不要**）。依存選定の実測結果と再検討の条件は [7.9.syntax_highlighting.NOT_ADOPTED.md](7.9.syntax_highlighting.NOT_ADOPTED.md) 参照 | — | — |
| 7.10 | **SSH/SFTP対応**（将来） | `[ ]` | リモートファイルシステム（大規模追加）（旧7.8） | ⭐⭐ | TBD |
| 7.13 | **ゴミ箱の自動削除（期限切れアイテムの定期パージ）** | `[ ]` | **2026-08-05: 7.7 Task 17 からスコープアウト**。`TrashConfig.auto_empty_days` は設定項目として存在（デフォルト `0`=オフ）だが未消費。当初案は Phase 7.5（バックグラウンドポーリング）にトリガーを相乗りさせる想定だったが 7.5 が未着手のため保留。**着手前に検討すべき問題**: (1) トリガー設計（起動時1回 vs 7.5 相乗りの真の定期実行）、(2) `.rwf-trash` fallback 側の age-filter 未実装（`purge_fallback_dirs_sync` は現状 `older_than_days` を一切見ない — `OsManaged` 側の `purge_os_trash_sync` とは非対称）、(3) **この機能が本当に必要か**（外部の cron / タスクスケジューラから本アプリを呼び出す形でも代替可能なため、内蔵機能化する価値があるか要検討）。7.5 完了後、または (3) の結論が出てから着手 | ⭐⭐ | 未見積 |
| 7.16 | **診断: JobProgress の記録（オプトイン）** | `[ ]` | 7.15 の積み残し（plan §4.4 で設計したが未実装）。`event_receiver` が `JobEvent::Progress`/`ProgressWithDetail` を `Transition::UpdateJobProgress`/`UpdateJobProgressWithDetail` にマップするため、これらは他の Transition と同様 `update_state` を通り **無制限に** `events.jsonl` へ記録される。大きなコピー1回で数千レコードになりうる。**方針（2026-08-12 決定）: 常時記録はリソース過多につき採用しない。必要なときだけ有効化するオプトインとする。** 設定 `Diagnostics.CaptureJobProgress`（デフォルト `false`）を追加し、無効時は該当 Transition の記録をスキップする。**実装上の注意**: job_id ごとのスロットリングは `Mutex<HashMap>` を観測パス（設計上ロックフリーを維持する箇所）に持ち込むため不可。`is_active()` ガードの後段でグローバルな `AtomicU64`（最終記録時刻）1個で間引くこと。判定は `format!("{:?}")` の**前**に安価な discriminant 比較で行い、無効セッション時のコストをゼロに保つ。詳細は [7.15.diagnostic_report.md](7.15.diagnostic_report.md) §4.4 および「Findings」節 | ⭐⭐ | 小規模 |
| 7.19 | **診断: スナップショットへの `viewer_anchor_pane` 記録** | `[x]` | 7.15 の積み残し。`snapshots/NNN-*.json` の state projection に SideBySide ビューアのアンカーペイン（`ui.layout.viewer_anchor_pane`）が含まれないため、バンドル解析時に「どちらのペインが表示されていたか」を `.txt` のレンダリング結果から推測するしかない（実例: 診断バンドル `20260908-212740` の SideBySide ペイン情報行バグ調査。viewer が左・ファイルペインが右という配置を ASCII アートから読み取る必要があった）。`ui.layout.viewer_layout` は既に記録済み（`rwf-lib/src/diagnostics/state_snapshot.rs:89`・`:240`）なので、同じ `layout` セクションに `viewer_anchor_pane` を1フィールド追加するだけで済む。**実装完了（2026-09-09）**: `LayoutSnapshot.viewer_anchor_pane` を追加。着手の決め手は診断バンドル `20260909-172206`（タブ往復でアンカーが他タブへ漏れる不具合）で**同じ推測作業を再度強いられた**こと — 2 回連続で `.txt` からアンカーを読み取る羽目になった | ⭐⭐ | 小規模 |
| 7.17 | **複数行テキスト入力ダイアログ（診断レポート記述欄ほか）** | `[x]` | **実装完了（2026-08-12）**。`rwf-bin/src/ui/multiline_text_input.rs`（ウィジェット）+ `MultiLineInput` ダイアログバリアント。7.15 の終了時レポート入力へ適用済み。**実機ドッグフィードバックによる設計変更（2026-08-12）**: 初版は長い行を**横スクロール**する実装だったが、実使用で却下 —「1行だけが横に流れるのは直感に反する」「画面外に続きがあるか分からない」。**ソフトラップ + `↵` マーカー**へ変更した。最右列を予約し、`↵` があれば実際の改行、無ければ折り返し行と判別できる（バグ報告を他人が読むため、入力した改行と折り返しの区別が必要）。代償として Up/Down は**視覚行**単位の移動になり、描画と移動が折り返し位置を共有する必要があるが、両者とも `visual_rows()` を呼ぶ単一実装なので不整合は起こらない。確定キーは **`Ctrl+S`（全プラットフォーム）** と `Ctrl+Enter`（Windows のみ判別可能）。rwf は crossterm の keyboard enhancement flags を有効化していないため、Unix 端末では Ctrl+Enter が Enter と区別できず、`Ctrl+S` が無ければ Linux/macOS で**送信不能**になる。7.14（fish 風補完）は別ファイル `text_input.rs` を触るため独立 | ⭐⭐⭐ | 完了 |
| 7.18 | **Linux 移植バグ（CI Linux ジョブが検出）** | `[x]` | **2026-09-08、Linux CI ジョブ追加時に検出・チケット化**。Windows 専用 CI では見えなかった 2 件。両方とも該当テストを `#[cfg_attr(unix, ignore = "...")]` で **ignore 表示**にしてある（cfg で消すと存在ごと見えなくなるため、`cargo test` の出力に理由付きで残す方を選択）。**(1) create_link スナップショットのプラットフォーム分岐**: `CreateLinkDialog::all_kinds()` は Windows のみ Junction を含む 3 種、Unix は 2 種なので描画が異なり、コミット済みスナップショットは Windows 版。対処案はスナップショット名に `_windows`/`_unix` サフィックスを付けプラットフォーム別ファイルにすること（Linux 版スナップショットは CI で `.snap.new` を artifact 化して回収する必要あり）。対象: `rwf-bin/src/ui/dialog/snapshot_tests/create_link.rs` の 3 テスト。**(2) Unix の fallback ゴミ箱が書き込み不可**: `backend/trash.rs` の `volume_root()` は `path.ancestors().last()` を返すため Unix では常に `/` になり、`/.rwf-trash` の作成が `Permission denied (os error 13)` で失敗する。**テストだけの問題ではなく実害のあるバグ** — `move_to_trash_sync` は OS ゴミ箱が失敗したとき自動的にこの fallback に落ちるため、非 root ユーザーでは Unix の fallback ゴミ箱が一切機能しない。対処案は (a) `volume_root` を st_dev 比較で真のマウントポイントまで遡らせる、(b) それが書き込み不可なら `$XDG_DATA_HOME/rwf/trash` へ退避。ただし `fallback_move_to_trash` は `fs::rename` 固定なのでファイルシステムを跨ぐ場合は copy+delete の追加が必要、かつ `purge_fallback_dirs_sync` / `EmptyTrash` も同じアンカーを見ているため同時に更新が要る。対象: `backend/local.rs` 2件・`backend/trash.rs` 2件・`job/job_executor.rs` 1件のテスト。**実装（2026-09-11）— Windows で検証済み・Linux は CI 待ち**。**(1)** プラットフォーム別スナップショットは採らず、`CreateLinkDialog.kinds`（表示する種類の一覧）をモデルに持たせた。レンダラと `cycle_kind` がそれぞれ持っていた `all_kinds()` の重複も解消。`create_link_two_kinds_*` スナップショット 3 件は Unix レイアウト（Junction 無し）を明示的に組み立てるので **両 CI ランナーで同一描画・両方で実行**され、Linux 生成物を回収する必要が無い。Junction を含む 3 件は `LinkCreateKind::Junction` が Windows にしか存在しないため Unix で ignore のまま（理由を「バグ」から「設計上」に更新）。**(2)** `backend/trash.rs`: fallback 先を候補列（ボリュームルートの `.rwf-trash` → ユーザー別 `dirs::data_local_dir()/rwf/trash`）にし、移動前に失敗した候補だけ次へ回す（`FallbackFailure::{NotMoved, Moved}` で区別 — 途中まで動いた後の再試行でファイルを失わない）。`rename` が別ファイルシステム（EXDEV / `ERROR_NOT_SAME_DEVICE`）で拒否されたら **コピー → メタデータ記録 → 元を削除** の順（どの段で失敗しても完全なコピーが 1 つ残る）。復元も同じ経路。`purge_fallback_dirs_sync` / `list_trash_sync` / `LocalFilesystemBackend::scan_trash` は共通の `fallback_dirs(roots)`（各ルートの sidecar ＋ユーザー別ディレクトリ）を掃くので移動先と食い違わない（ガードテスト `a_move_into_any_candidate_is_found_by_the_sweep`）。ignore 5 件を解除、新規テスト 5 件（候補の素通り・全候補不可でファイルが残る・コピー＋削除でツリーが保たれる・sweep の網羅と重複排除）。**残: Linux CI での実行確認**（WSL 無しのためローカルで cfg(unix) 経路は未コンパイル）。検証（Windows）: `fmt --check` / `clippy --all-targets -D warnings` 緑、rwf-bin 352+4+8、rwf-lib 1321 passed（`--skip propert`）。**Linux CI で完了確認（2026-09-11、run `34607332535`）**: `build + clippy (linux)`・`test (linux)` とも緑 — rwf-bin 346+4+9、rwf-lib 1450 passed / 0 failed。旧 ignore 5 件（`backend/local.rs`・`backend/trash.rs`・`job/job_executor.rs`）と新規 fallback テスト 5 件、`create_link_two_kinds_*` 3 件がいずれも Linux で**実行されて** ok（ログで個別確認）。残る ignore は設計上の Junction スナップショット 3 件と破壊的な `test_purge_os_trash_purges_item` のみ | ⭐⭐⭐ | 完了 |
| 7.14 | **fish シェル風インライン自動補完（ダイアログ内テキスト入力）** | `[ ]` | rwf 内の各種ダイアログのテキスト入力（ディレクトリ作成 `Create Directory`、シンプルリネーム等、`Dialog::input()` を使う汎用 `InputDialog`）に、fish シェルのような「入力中に薄字で候補を先読み表示し、Tab/→/End 等で確定」インライン補完を追加。候補ソースはカレントディレクトリのファイル/フォルダ名一致（パス系入力）や履歴（HistoryDialog は既存）を想定。Jump to Path（Phase 2.1）は既に非同期パス補完を持つが専用ダイアログ限定。本タスクは汎用 `InputDialog` 側への一般化が主眼。CLI補完（bash/zsh/fish 等のシェル統合スクリプト生成）とは別物 — 混同しないこと | ⭐⭐⭐ | 未見積 |
| 7.20 | **クリップメニューの命名整理とメニュー説明表示** | `[x]` | **2026-09-10、診断バンドル `20260910-004934` のレポート後半からチケット化**（前半の「clip\* メニューが全滅」バグは別途修正済み）。ユーザー報告は「メニュー名が分かりにくい、見直したい」。実体は 2 つ。**(1) 命名**: クリップ項目は「何を（名前/フルパス/フォルダ）」「どれを（カーソル/マーク済み）」「どう（素/引用符付き）」の 3 軸を持つが、名前がこれを一貫してエンコードしていない。最悪なのは `clip file path`（ファイルの絶対パス）と `clip path`（ペインのディレクトリ）が 1 語違いであること。さらに `clip` / `copy` / `Copy`（=`C` のファイルコピー）の 3 語彙が混在（関数名は `menu_clip_paths` なのにロードするファイルは `menu_copy_paths.json`、説明は "Show copy-paths menu"）。**(2) 説明文が既にあるのに表示されていない**: 各 `CustomFunction` は `Name` より遥かに明確な `Description` を持つ（`clip path` に対し "Copy the active pane's directory"）が、`CustomFunctionMenuDialog` は `MenuItem{name, action}` しか持たずレンダラに届かない。**方針: 両方やる** — 改名だけでは意味を約 25 文字に詰め込む必要があり、それが今回の失敗そのもの。`MenuItem.description` を追加し `resolve_menu_files` で設定ロード時に一度だけ解決、メニューを 2 カラム描画にする。**副産物**: `ui/dialog/mod.rs` のメニュー幅計算が `i.name.len()`（バイト長）を使っており日本語メニュー名でダイアログ幅がずれる既存バグを同時に修正（`unicode_utils` へ）。**移行**: `--export-config-files` は `write_if_absent` なので既存の `%APPDATA%\rwf\` は一切触らない — 実機設定はユーザーが手動移行（対応表は設計書 §6）。詳細は [7.20.clip_menu_naming.md](7.20.clip_menu_naming.md) 参照。**レビュー完了（2026-09-10）**：ラベルは`file name` / `full path` / `current directory` / `marked file names` / `marked full paths` の 5 行に確定（メニュー題名が `clip` なので各行の `clip` は冗長。先頭文字が差分になるので char-jump も初めて機能する）。引用符付き 2 件はメニューから外してキーバインドへ（`ClipText` は元々直接バインド可能。**引用を修飾キー化してはいけない** — テンプレート側に既に `"` があるため二重引用になる）。**`current directory` の命名は 2 案を却下して決着**: `… name` は隣の `file name`（basename）の手前 basename を約束してしまうが `$P` はフルパス。`parent directory` は rwf 既存の語彙と衝突する（`Backspace: NavigateToParent` = 一階層上。`$P` は上ではなく現在地）。**将来の候補**: `$P` の末尾セグメントを返すマクロ（仮 `$PN`）は未実装だが有用（カレントフォルダ名でアーカイブを命名等）。入れるなら `current directory` の下に `current directory name` を**追加**する形で。**ピース 2・3 実装完了（2026-09-10、`f0682d6`）** — リソース JSON・キーバインド・`help_content.rs`/`lib.rs`/`main.rs` の参照・`docs/USER_GUIDE.md` を更新し、メニュー項目の `Action` が実在の関数に解決することを検証するガードテスト 2 件を追加（実際に旧 `Action` で落ちることを確認済み）。~~残りはピース 1（説明文の 2 カラム表示）のみ~~ **ピース 1・4 実装完了（2026-09-11）— 7.20 完了**。`MenuItem.description`（`#[serde(default, skip_serializing_if)]`）を追加し、`resolve_menu_files` の末尾で `fill_menu_descriptions` が関数の `Description` を一度だけ埋める（メニューファイル側の明示 `Description` が優先、組み込みアクションは `None`、リロードで再実行）。`render_custom_function_menu` は名前列＋薄字の説明列の 2 カラム描画、選択行は新定数 `DIALOG_SELECTED_DIM` でハイライトを途切れさせない。説明列が 20 桁未満になる幅では列ごと省略（全説明が収まるなら常に表示）。幅計算は `menu_content_width`（表示幅）に一本化し、`OpenWithPicker` の `.len()` も表示幅へ（ピース 4）。**既存スナップショット 4 件は無変更**（説明の無い項目は従来と同一描画 — §4 の「再承認が必要」は結果的に不要）。新規: lib テスト 4 件（関数から補完／メニュー側優先／解決不能は `None`／出荷ファイルを実ローダで読むと clip メニュー全項目に説明が付く）、スナップショット 3 件（2 カラム・CJK 名・狭幅で説明列省略）。`docs/USER_GUIDE.md` に説明表示とメニュー側上書き・リロードで反映を追記。検証: `fmt --check` / `clippy --all-targets -D warnings` 緑、rwf-bin 341+4+8、rwf-lib 1289 passed（`--skip propert`）。詳細は設計書末尾の Implementation notes | ⭐⭐ | 完了 |
| 7.21 | **メインスレッド上の同期 I/O 監査（残り）** | `[x]` | **2026-09-10、7.20 の黒画面調査（診断バンドル `20260910-203646`）から派生して全体監査**。「ジョブを経由しない同期 FS アクセス」を rwf-lib/rwf-bin 全体で洗い出した結果。**state/handlers は完全にクリーン**（`update_state` 系に FS 呼び出しゼロ — アーキテクチャは守られている）。残る違反は 2 系統。**(A) 描画パス内**（最悪 — フレーム毎に走る）: (A1) `rwf-bin/src/ui.rs:165` — SideBySide ビューアのディレクトリプレビューが `std::fs::read_dir` を **`render_ui_inner` の中で** 実行し子要素を全数え上げ。コメントは「ローカルFSならサブミリ秒、かつ描画は状態変化時のみ」と書くが**両方とも誤り** — カーソルはネットワーク共有上のディレクトリに乗りうるし、ジョブ実行中は「状態変化時のみ」が成り立たない（旧コードは毎イテレーション再描画を強制していた＝実測 154fps。7.20 でスピナー間隔に律速したので現在は約 7fps だが、設計上の欠陥は残る）。(A2) `rwf-bin/src/ui/dialog/jump_to_file.rs:182` — 表示中の候補**1行ごとに** `is_dir()`。N stat／フレーム。**(B) ユーザー操作起点の一発もの**（許容範囲だが UNC でハングしうる）: `ui/dialog/confirm.rs:456-528`（Jump to Path の Enter 検証）、`state/handlers/ui.rs:584` の `volume_info::get_all_drives()`（ドライブダイアログ。ネットワークドライブを列挙）、`job/undo_preflight.rs:84,93`、`pipe_to_action.rs:37,65`。**(C) 不可避＝対処不要**: 起動時の config/keybindings/session 読み込み（`%APPDATA%` 配下の小さなローカルファイル。これ無しでは起動できない）、`logging.rs` のローテーション、`diagnostics/mod.rs:292` のセッションディレクトリ命名、`model/search.rs:141` の migemo 辞書パス確認。**進捗（2026-09-10）**: **A1・A2 修正済み** — A1 は `JobKind::CountDirectoryEntries` + `AppState.dir_preview_counts` へ、A2 は収集ジョブが既に持っていた `is_dir` を `SuccessData::JumpCandidates` で運び `JumpToFileDialog.dir_paths` に格納。再発防止に契約テスト `the_render_path_does_not_touch_the_filesystem`（`rwf-bin/tests/repo_contracts.rs`）を追加し、描画パス配下の FS 呼び出しを機械的に禁止（`ui/dialog/confirm.rs` は描画関数を1つも持たない Enter ハンドラなので明示除外）。**残りは B のみ**。旧方針: A1・A2 を先に（描画パスから FS を追い出す — プレビュー件数は `CalculateSize` 同様ジョブ化するかキャッシュ、`is_dir` は候補生成時に確定させて `JumpCandidates` に持たせる）。B は「UNC でハングした」実例が出てから。**B も完了（2026-09-11）— 7.21 完了**。4 件すべてジョブ化: (B1) Jump to Path / Jump to File の Enter 検証から `is_dir()`/`is_file()` を撤去 — 入力はファイルシステムに問い合わせず `resolve_typed_path`（`has_root()` 基準）で解決し、存在しなければ `ReadDirectory` が失敗して 7.23 の `ResolveFallbackPath` が最寄りの祖先へ着地させる（**挙動変更**: 以前は存在しない入力で Enter しても無反応だった）。Jump to File のディレクトリ判定は `dir_paths`（収集ジョブ分に加え、開いた時点のペインのエントリから高速候補分も埋める）と末尾区切り文字で行う。(B2) `JobKind::ListDrives` — ドライブダイアログは即座にホーム＋ネットワーク共有で開き、システムドライブはワーカーから追記（取得中は「listing drives…」行、Esc でジョブ取消）。(B3) `JobKind::ResolvePipeTarget` — カスタム関数の出力パスの stat を `update_state` から追い出した（`ClipText` は判定不要なので直接）。移設時に `JumpToPath` 分岐へ `active_job_id` の設定を追加（ReadDirectory 契約）。(B4) `JobKind::PreflightReversal` — Undo/Redo の事前検証を Enter から外し、完了時に `ExecuteReversal` 開始（背景ジョブ名「Undo Copy」も state 側で登録）か blocked 行サマリ表示。**再発防止**: 契約テスト `state_transitions_do_not_touch_the_filesystem`（`rwf-lib/src/state/handlers` を FS トークン＋既知のブロッキングヘルパ `get_all_drives(`/`process_pipe_to_action(`/`preflight_check(` で走査）を追加し、**実際に `get_all_drives()` を仕込んで落ちることを確認済み**。`docs/IMPLICIT_CONTRACTS.md` に §8 として描画パス契約と併せて記載。残り（対処しない判断）: Create File/Directory の名前検証の `symlink_metadata`（既に一覧済みのペイン内 1 stat）、起動時の設定読み込み（カテゴリ C）。テスト: lib `main_thread_io_tests` 7 件、bin Jump 3 件・ドライブ取得中スナップショット、Operation Report の既存 2 件はジョブ完了を経由する形に更新。検証: `fmt --check`/`clippy --all-targets -D warnings` 緑、rwf-bin 356+4+9、rwf-lib 1328 passed（`--skip propert`） | ⭐⭐ | 完了 |
| 7.22 | **起動時ジョブの可視化とエラーダイアログの情報設計** | `[x]` | **2026-09-10、診断バンドル `20260910-203646` の項目(3)(4)からチケット化**（同バンドルの(1)(2)は 7.23 で修正済み）。**(3) ディレクトリ読み込みが実行中まったく見えない**: タブバーのスピナー（`ui/tab_bar.rs:67`）もタスクパネルも `background_jobs` を見ているが、`ReadDirectory` はそこに登録されない（`state.jobs` のみ）。バンドルの `000-start.json` が動かぬ証拠 — `jobs.active` に Running 2件・両ペイン `is_loading=true` なのに `jobs.background=[]`、画面は「No active tasks」でタブ5 にスピナーなし。**間欠的な現象ではなく `ReadDirectory` は常に不可視**。スピナー描画側は既に正しく書かれており、渡すものが無いだけ。**実装時の罠（必読）**: タブのキーが2種類あり両方 `usize` — `requesting_pane` は tab **id**（`t.id == ...` で照合）、`background_jobs` は tab **index**（`get_active_job_count(idx)`、ログの `[Tab {}]` も `tab_id + 1`）。素朴に `requesting_pane.0` を渡すとタブを閉じた後に**別のタブにスピナーが出る**。推奨は `background_jobs` 側も id キーに統一（index は閉じると別タブを指す＝`498c710` で既に踏んだバグ種）。ついでに `with_requesting_pane` の引数名 `tab_idx` は嘘（id が渡る）なので改名。**(4) "Operation Failed" が不適切**: 起動時はユーザーは何も操作していない。「何が・どこで・いつ・なぜ」の4枠に再設計し、`[Retry]`/`[Open parent]`/`[Dismiss]` を持たせる。**その前提として実バグが1件** — `from_job_failure_in` のエラー種別判定が **英語部分文字列マッチ**（`contains("not found")`）なので、日本語 Windows では `ネットワーク名が見つかりません。` が一致せず**常に既定の "Operation Failed"** に落ちる。`io::ErrorKind` と OS エラーコード（`os error 67` = `ERROR_BAD_NET_NAME`）で発生源において構造化すべき。また「いつ」を言うには `JobSpec.origin`（`SessionRestore`/`UserAction`/`Refresh`）が必要 — 現状どこにも情報が無い。詳細・実装順・引き継ぎ注意点は [7.22.startup_job_visibility.md](7.22.startup_job_visibility.md) 参照。**実装完了（2026-09-11）— §4 の 5 ピース全て**。(1) `job/failure_kind.rs` の `FailureKind` — OS エラーコード（SMB の 67/53 等は自前表）→ `io::ErrorKind` の順で分類。**設計からの逸脱**: 種別を `OpResult::Failed` に載せず、std が付ける非ローカライズの `(os error N)` サフィックスからコードを復元して同じ分類にかける（`Failed(String)` は約 80 箇所で構築されるため）。std の書式変更はガードテストが先に落ちる。(2) `ReadDirectory` を `App::submit_job` で **quiet な** background job として登録（`track_directory_read`）、`background_jobs` を **tab id キーに統一**（案 b。`app.rs` 4 箇所・`job_management.rs` 2 箇所・`tab_bar.rs`・ログの `[Tab N]`・ジョブマネージャ詳細）。`with_requesting_pane` の引数名も `tab_id` へ。**配線中に発見した実バグ**: `AcknowledgeCancel` が `background_jobs` に取消を伝えていなかった — 読み込みを登録した瞬間、ナビゲーションで置き換えられた全読み込みがタブで永久に回るところだった。(3) quiet 期間 500ms（`Job::started_at` 起点＝ワーカーが拾ってから）を過ぎた読み込みだけ `AnnounceQuietJobs` で 1 回 `Started` を出し、以後は通常どおり結果を記録。未告知のものは失敗時のみ記録。**逸脱**: `SessionRestore` も quiet 期間の対象（起動毎に 20 行出ると詰まった 1 本が埋もれる。スピナーは即時）。(4) `JobSpec.origin`（`SessionRestore`/`UserAction`/`Refresh`）、フォールバックジョブへ継承。(5) `ReadFailureDialog`（`DialogContent::ReadFailure`）: 題名は種別（*Directory Unavailable* 等）、「Tab 5, right pane — while restoring your session」、パス単独行、OS の原文（バックエンドの前置きは除去）、ヒント、`[Retry]  [*Dismiss*]`（既定フォーカス Dismiss、`frame.rs` に `default_button_index`）。Retry は `Transition::RetryPaneRead`（非アクティブタブでも可）。**逸脱**: `[Open parent]` は無し（この dialog はフォールバックが祖先を 1 つも読めなかった時にしか出ない）、検索中にタブが閉じたらモーダルではなくタスクパネルに `[FAIL]`。テスト: lib `pane_read_visibility_tests`（10 件）・`failure_kind`（9 件）・`read_failure`（4 件）・`error_handling_tests` 追加 5 件、bin 起動読み込み登録・Retry/Dismiss 確定・入力・スナップショット 3 件×2 サイズ。検証: `fmt --check`/`clippy --all-targets -D warnings` 緑、rwf-bin 349+4+8、rwf-lib 1316 passed（`--skip propert`） | ⭐⭐⭐ | 完了 |
| 7.23 | **メインスレッド非ブロッキング化とクリップボード経路の修正** | `[x]` | **実装完了（2026-09-10）**。診断バンドル `20260910-004226` / `20260910-004934` / `20260910-203646` の3件から判明した5件をまとめて修正。**(1) clip\* メニューが全滅**（`20260910-004934` seq 48 `Failed("SetClipboard reached worker pool unexpectedly")`）: `SetClipboard`・`SuspendAndRun`・`ExecuteCustomFunction{suspend:true}` はメインスレッド専用だが、それを知るのは `App::run` の `pending_job_submission` ドレインだけ。ダイアログ確定経路が `pool.submit_job` を直接呼んでバイパスしていたため、**キーバインド直接起動を除く全 clip 経路**（F6メニュー・コンテキストメニュー・`T` セレクタ・`$I` 入力）が死んでいた。`App::submit_job` という単一の投入口を作り全箇所を通した。副次的に `PipeToAction: ClipText` と補完由来の `jobs_to_start`（`app.rs:488`）も同じバイパスをしており、かつ `jobs.start_job` を呼んでいなかった（＝`CompleteJob` が `jobs.active` に spec を見つけられず**ペイン更新もダイアログも全部スキップ**していた）。**(2) 起動時の黒画面**（`20260910-203646`）: `session::resolve_restored_location` が復元パス10本＋その祖先すべてに同期 `exists()` を撃っており、`TerminalManager::new()` の代替画面入り後・初回描画前に実行されるため、死んだ SMB 共有1本で名前解決タイムアウト×祖先数ぶん真っ暗になっていた。**復元はもう一切 FS を触らない**（verbatim 復元）。祖先探索は `JobKind::ResolveFallbackPath` としてワーカーへ移し、ペインの `ReadDirectory` が失敗して初めて走る。見つかればそこへ移動＋タスクパネル通知（モーダルなし）、何も生きていなければ**元のエラー**でダイアログ（`pending_read_failures` で往復越しに保持）。**(3) ビジーループ**: 同バンドルで 8.4 秒間に **Render 1292回・Wake 0回**（約154fps の空回り）。`has_active_jobs()` が毎イテレーション `ui_needs_update` を立ててタイムアウトを0にしていた。`Display.SpinnerFrameMs` に律速。**(4) 失敗ダイアログがどのペインか言わない**（`20260910-004226`）: `spec.requesting_pane` に tab id とペイン側が入っているのに捨てていた。`Dialog::from_job_failure_in` で「Tab 5 right pane: ...」と前置。**(5) 失敗した `ReadDirectory` が `active_job_id` を解放しない** — `is_loading` だけ落として job id を残すため、スナップショットが「まだ動作中」の形（docs/DIAGNOSTIC_BUNDLES.md の読み方）に見えていた。テスト: rwf-lib 1415・rwf-bin 338+4+8 全緑 | ⭐⭐⭐ | 完了 |

---

## Future Enhancement Candidates (Phase 8+検討事項)

> Phase 7 完了後に検討すべき高付加価値機能。

| 機能 | 詳細 | 可能性 |
|------|------|--------|
| **ディスク使用量可視化（グラフ）** | サイズ計算結果を円グラフ/棒グラフで表示。ncdu 風のビジュアル分析 | Phase 8.1 |
| **永続サイズキャッシュ** | `~/.rwf/size_cache.json` に計算結果を保存。ディレクトリ mtime で無効化判定 | Phase 8.2 |
| **動的ペイン幅調整** | マウス・キーでペイン幅を変更（左右均等分割 → カスタム比率） | Phase 8.3 |
| **Escape キャンセル** | バックグラウンドジョブ実行中に Escape で即座にキャンセル | Phase 8.4 |
| **Git ステータス表示** | ペイン内で Git ファイル状態（modified/staged等）を色分け表示 | Phase 8.5 |
| **Registered Folder へのコピー/移動** | **CopyToRegisteredFolder** / **MoveToRegisteredFolder**。大量の登録フォルダから高速に絞り込み・選択して整理する機能 | Phase 8.6 |
| **ユーザー定義マジックバイト表** | Phase 7.3 の内蔵シグネチャテーブル（手組み・約20〜30形式）でカバーしきれない形式向けに、`file_type_map.json` と同じ外部JSONパターンで拡張可能にする | Phase 8.7 |
| **libmagic 統合（オプション）** | `magic`クレート経由の完全なマジックバイト判定。libmagicがインストール済みの環境でのみ有効化（Phase 5.6 の compress-tools 同様の任意依存パターン）。Phase 7.3 では採用見送り（Windows非対応のため） | Phase 8.8 |
| **ファイル一覧ペインへの常時タイプ表示** | Phase 7.3 では File Information ダイアログでのオンデマンド表示のみ。常時アイコン/列表示は都度I/Oコストが発生するため別途検討 | Phase 8.9 |
| **migemo 統合方式の見直し（クレート優先 + 外部モジュール fallback）** | Rust の migemo クレートは Linux でのビルドに失敗することが判明。クロスプラットフォーム対応のクレートを優先利用しつつ、ビルド不可な環境（Linux/macOS）では TWF プロトタイプで動作実績のある `migemo.dll` 等の外部モジュール方式（辞書ファイル同梱）へフォールバックできないか検討する。TWF プロトタイプでは Windows/Linux 双方で動作実績あり | Phase 8.10 |
| **7-Zip 外部モジュール対応（アーカイブ形式拡張）** | 現行の 7z サポート（Phase 5.1、`sevenz-rust` クレート）は対応アーカイブ形式が限定的。TWF プロトタイプで動作実績のある 7-Zip 外部モジュール（`7z.dll`/`7za` 等）が利用可能な環境ではそちらを優先利用し、より多くの形式（RAR/LZH等、5.3・5.5・5.6 参照）に対応できないか検討する | Phase 8.11 |

> 旧 8.7 コマンドパレットは 2026-07-05 に Phase 7.2 へ昇格。マジックバイト判定（旧 8.7 再割当分）は
> 2026-07-18 に Phase 7.3 スマート・ファイルオープナーへ統合済み（[7.3.smart_file_opener.md](7.3.smart_file_opener.md)）。
> 上記 8.7〜8.9 はその設計で意図的にスコープ外とした派生候補。

---

## テスト戦略

### フェーズ共通方針
```
ダイアログ系:    レンダリング出力の snapshot テスト
ファイル操作系:  assert_fs + tempfile による実FS上の統合テスト
非同期系:        tokio::test による並行処理・キャンセルテスト
エラー系:        権限不足・容量不足などの異常系パス
プロパティ:      proptest による状態遷移の網羅テスト（既存パターン踏襲）
```

### 現状テスト数: 1043件（rwf-lib 単体 + プロパティ + 統合、2026-07-02 実測）＋ rwf-bin UI テスト

> 実行時間の目安: rwf-lib 全件を `--test-threads=1` で約37分。通常はテスト名フィルタで対象のみ実行すること
> （実行手順の規約は `.claude/CLAUDE.local.md` を参照）。

---

## 優先度・スケジュール感

```
Phase 1 (〜2週間)  → UIダイアログ完結、日常操作の安定化
Phase 2 (〜2週間)  → ナビゲーション強化
Phase 3 (〜3週間)  → ジョブ管理UI洗練
Phase 4 (〜3週間)  → ビューア完成（テキスト+Hex）
Phase 5 (〜2週間)  → アーカイブ拡張
Phase 6 (〜4週間)  → twf完全パリティ
Phase M (完了)     → 品質整備（機能凍結・解除済み。Phase 7 残タスクの前提）
Phase 7 (再開)     → 差別化機能
```

**Phase 1〜3完了**: 日常ユースケースでtwfと同等  
**Phase 4〜6完了**: 全機能でtwfパリティ達成  
**Phase 7**: rwf独自の強み確立

---

## セッション再開時の確認事項

- 現在のフェーズ・タスク番号
- 最後に完了したタスク
- 残課題・ブロッカー

最終更新: 2026-09-11（**7.18・7.20・7.21・7.22 完了**。7.20 はメニュー説明の 2 カラム表示（`6248a88`）、7.22 は読み込みの可視化・構造化エラー種別・読み込み失敗ダイアログ（`a88d509`）、7.18 は Unix の fallback ゴミ箱と create_link スナップショットの移植性（`8123f84`）、7.21 はカテゴリ B 4 件のジョブ化とハンドラ層 FS 契約テスト。いずれも `fmt --check`/`clippy -D warnings`/rwf-bin 全件/rwf-lib `--skip propert` 全件緑。push 済み、7.18 は Linux CI（run `34607332535`）で緑を確認して `[x]`）
前々回更新: 2026-09-10（**実機ドッグフィード 3 バンドルの一括対応**。診断バンドル `20260910-004226` / `20260910-004934` / `20260910-203646` を解析し、**5 件を修正して 7.23 として完了**（clip\* メニュー全滅・起動時の黒画面・154fps ビジーループ・失敗ダイアログがペインを言わない・失敗 `ReadDirectory` が `active_job_id` を解放しない）。併せて「ジョブを経由しない同期 FS アクセス」を全体監査し **7.21** として起票、うち描画パス内の 2 件（`ui.rs` の `read_dir`・`jump_to_file.rs` の行ごと `is_dir()`）を修正済み。**再発防止の契約テスト `the_render_path_does_not_touch_the_filesystem` を追加**（`rwf-bin/tests/repo_contracts.rs`）。同バンドルの残り 2 項目を **7.22** として起票、クリップメニューの命名整理を **7.20** として設計完了・実装未着手。検証: `cargo fmt --check` / `clippy --all-targets -D warnings` / rwf-lib 1415 / rwf-bin 338+4+8 いずれも全緑）
前回更新: 2026-08-17（**7.6 Undo/Redo（Operation Report）実装完了**。詳細は下の「前々回」以前の記述および Phase 7 表を参照）
現在のフェーズ: **Phase 7**（機能開発再開。Phase M1〜M7 全完了、7.1 Leap Navigation・7.3 スマート・ファイルオープナー・7.6 Undo/Redo・
7.7 スマート・トラッシュ・7.11 属性/タイムスタンプ変更・7.12 Create Link/Create File・7.15 デバッグレポート機能・7.17 複数行テキスト入力・7.19 診断スナップショット拡張・7.20 クリップメニュー命名整理・7.21 同期 I/O 監査・7.22 起動時ジョブの可視化・7.23 メインスレッド非ブロッキング化・7.18 Linux 移植バグ は完了済み）
Phase 6: 全タスク完了（6.1 の colors.json 分離のみ後フェーズ送り）
Phase M: 全タスク完了（詳細: [quality_overhaul.md](quality_overhaul.md) の「Phase M 完了サマリ」）

**（解決済み・経緯として保存）7.20 が半端に手適用されビルドが通らなかった件（2026-09-10）**
`rwf-lib/resources/` に 7.20 のリネームが手作業で部分適用されている（`default_menu_copy_paths.json` 削除 → `default_menu_clip.json` 追加、`default_custom_functions.json` と `default_keybindings.json` を編集）。**コード側が追随していないため `cargo build` が失敗する。** 再開時はまず以下を潰すこと。設計の正は [7.20.clip_menu_naming.md](7.20.clip_menu_naming.md) §2.1。
1. **ビルド不成立（機械的）**: `rwf-lib/src/help_content.rs:53` の `include_str!("../resources/default_menu_copy_paths.json")` が削除済みファイルを指す。`DEFAULT_MENU_COPY_PATHS` → `DEFAULT_MENU_CLIP` へ改名し、`rwf-lib/src/lib.rs` の re-export と `rwf-bin/src/main.rs` の `export_default_configs` のファイル名も合わせる。
2. **ラベルが 1 個ずれている（最重要・設計の目的を無効化する）**: 現在の割り当ては `$P$/$F` → `file path`、`$P` → **`full path`**。**`full path` がディレクトリを指してしまっている**。これは今回の作業がまさに潰そうとした `clip file path` と `clip path` の取り違えを、新しい語で再生産したもの（しかも "full path" はファイルの完全パスを強く示唆するぶん悪化している）。正しくは `$P$/$F` → `full path`、`$P` → `current directory`。
3. **F6 が死んでいる**: `default_keybindings.json` は `"F6": "clip menu"` だが、`default_custom_functions.json` のメニュー関数名は `"clip name"`（`clip menu` ではない）。**一致する関数が無いので F6 は無反応**。加えて `clip name` は各項目のラベルと紛らわしい。
4. **`default_menu_clip.json` に旧名の `Action` が残存**: `{"Name": "marked full paths", "Action": "clip marked file path"}` — この関数名はもう存在しないので**この項目は黙って何もしない**。7.20 §4 が「`Action` は関数 `Name` で引かれるため、片方のファイルだけ改名すると実行時に無言で壊れる」として**ガードテストを要求している、まさにその事故**。実装時は先にガードテスト（全 shipped メニュー項目の `Action` が実在の関数に解決すること）を書くこと。
5. **`Description` が未更新**: 「Show copy-paths menu」「Copy the cursor file's name」など旧文面・命令形のまま。7.20 §2.1 は名詞句へ書き換える設計（メニュー題名が動詞を供給するため）。ピース 1（説明文の 2 カラム表示）を入れると画面に出るので、同時に直すこと。
6. **引用符付き 2 件がメニューに残っている**: `Ctrl+F6` は追加済みだが `default_menu_clip.json` にも 2 項目が残る。§2.4 はメニューから外す設計。意図的に残すなら §2.4 を更新すること。
**✅ 上記の 6 件は解消済み（2026-09-10、`f0682d6`）**。半端に手適用されていたリソース JSON は一旦 HEAD へ戻し、7.20 §2.1 に沿って**テンプレート基準で**貼り直した（名前同士で対応づけると 1 個ずれる。これが元の事故）。現在: `$F`→`file name` / `$P$/$F`→`full path` / `$P`→`current directory` / `$MFL`→`marked file names` / `$MPL`→`marked full paths`、引用符付き 2 件はメニューから外し `Ctrl+F6` に割当、`Description` は名詞句へ書き換え、`help_content.rs`/`lib.rs`/`main.rs` の参照と `docs/USER_GUIDE.md` も追随。**ガードテスト 2 件を追加**（`shipped_menu_actions_all_resolve_to_a_shipped_function` / `shipped_menu_references_name_files_that_exist`、`model/dialog/mod.rs`）— 実際に旧 `Action` を差し戻して**落ちることを確認してから**緑にしている。**7.20 の残りはピース 1（説明文の 2 カラム表示）のみ** — ラベルを短くしたのはこれが来る前提なので、次に着手するならここ。

**⚠ 新セッションへの引き継ぎ（2026-09-10 時点）**:
- **作業ツリーはコミット済み**（`e06f8bf` 実装 / `238927a` 設計文書 / `f0682d6` 7.20 リネーム）。未 push。設計文書の記述より**コードが一次情報**なので、再開時は `git log -3` と `git show` で現物を確認すること。
- **7.20 は完了（2026-09-11）** — ピース 2・3（リネーム）に続きピース 1・4（説明文の 2 カラム表示・表示幅計算）も実装済み。
- **7.22 は完了（2026-09-11）** — 5 ピース全て。設計からの逸脱 4 点は設計書末尾の Implementation notes に記録。
- **7.21 は完了（2026-09-11）** — カテゴリ B の 4 件をジョブ化し、ハンドラ層の FS 契約テストを追加。
- **7.18 は完了（2026-09-11）** — push 後の CI run `34607332535` で `test (linux)` 緑を確認（`8123f84`）。

次の作業候補（優先順。実行順の一次情報は上の Phase 7 表の行順）:
1. ~~**7.20 クリップメニューの命名整理**~~ — 完了（2026-09-11）
2. **7.22 起動時ジョブの可視化** — 実機で「何が起きているか分からない」と報告された体験の core。§3.2 は実バグ
3. **7.2 コマンドパレット** — 意図的に後回し。着手前にコンテキスト絞り込み等の設計を詰める
4. **7.14 fish シェル風インライン自動補完** — 独立タスク。7.17 と同じ `text_input.rs` を触る
5. **7.16 JobProgress オプトイン記録** — 小規模。7.15 の積み残し

## ROADMAP外で実装済みの機能（2026-06-13〜07-02、要フェーズ整理）

- **シンボリックリンク/ジャンクション対応** — `LinkKind` enum、`symlink_metadata` による検出、一覧での `->` サフィックス表示、ファイル名バーの `name->target` 表示、File Information ダイアログの Type/Target 行（b414944〜fa2e5c1）
- **SuspendAndRun ジョブ** — ターミナルエディタ（vim 等）起動のための TUI サスペンド対応（ee3011b）
- **動的ヘルプビューア + `--export-config-files` + キーバインド衝突検出**（e406672、Phase 6.7 として完了扱い）
- **config status 表示の改善** — keybindings の「ファイル無し」とエラーの区別（6978949）
- **Input ダイアログのテキスト編集実装**（d87a383）
- **タスクパネル縮小順序の修正・スピナー共通化・LEAP バースピナー**（9e500c8, 13c7189, cad6bd7）

## 実装内訳アーカイブ

Phase 1〜4 の各タスクの実装内訳（コミット時点の詳細メモ）は
[docs/history/roadmap_implementation_notes.md](../docs/history/roadmap_implementation_notes.md)
へ移動した（2026-07-18、ROADMAP.md 肥大化対策）。

## 備考
- テスト実行の規約（OOM 対策・推奨コマンド）は `.claude/CLAUDE.local.md` の「Test Suite Status」を参照。
