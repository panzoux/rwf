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
| 7.3 | **スマート・ファイルオープナー（Rifle + コンテンツ判定）** | `[ ]` | Phase 6.2 (ExtensionAssociations) の発展形 + 旧 8.7（マジックバイト判定）を統合。複数候補オープン（Open With ピッカー）、拡張子/実体不一致の警告、拡張子未登録ファイルの検出フォールバック。**次に着手する機能**。詳細は [7.3.smart_file_opener.md](7.3.smart_file_opener.md) 参照 | ⭐⭐⭐⭐⭐ | 2週間 |
| 7.6 | **Undo/Redo（トランザクション・ロールバック）** | `[ ]` | Job履歴に基づく操作の取り消し・やり直し。Job + Transition 体系で逆操作を記録。詳細は [7.6.transactional_rollback.md](7.6.transactional_rollback.md) 参照。**rwf の "killer feature"** | ⭐⭐⭐⭐⭐ | 3週間 |
| 7.2 | **コマンドパレット** | `[ ]` | 旧 Phase 8.7 から昇格。ヘルプビューア（`?`）で検索して `Enter` でアクションを直接実行。VS Code の `Ctrl+Shift+P` と同じ体験。ヘルプはすでに検索ボックスとフィルタ済みリストを持っており、不足しているのは「ハイライト中のアクションをディスパッチして閉じる」`Enter` キーの処理のみ。コンテキスト（NormalMode / ViewerMode）でアクション絞り込みが必要。**2026-07-18: 意図的に後回し** — コンテキスト絞り込み・他機能との自然な統合（Open With ピッカー等）を含めた設計をまだ詰めていないため、7.3/7.6 の後に着手 | ⭐⭐⭐ | 小規模 |
| 7.4 | **バックグラウンド・ディレクトリサイズ計算** | `[ ]` | **Shift+S** で再帰的ディレクトリサイズを非同期計算。エントリごとにサイズを段階的に埋める。スピナー + Task pane ログ。詳細は [7.4.calculate_directory_size.md](7.4.calculate_directory_size.md) 参照 | ⭐⭐⭐⭐ | 2.5週間 |
| 7.5 | **バックグラウンドポーリング（Layer 2）** | `[ ]` | 可視エントリのメタデータ定期チェック（twf PerformSmartRefresh 相当）。間隔は config `polling_interval_ms`（1.4.2 で追加済み） | ⭐⭐⭐ | 2週間 |
| 7.7 | **スマート・トラッシュ（ゴミ箱）管理** | `[ ]` | Windows/macOS/Linux 各OS標準への対応。削除ではなくゴミ箱へ移動、復元サポート。詳細は [7.7.smart_trash.md](7.7.smart_trash.md) 参照 | ⭐⭐⭐ | 2週間 |
| 7.9 | **シンタックスハイライト（ビューア）** | `[ ]` | `syntect`クレートによるコードハイライト（twfにない）。テキストビューア拡張（旧7.6） | ⭐⭐⭐ | 2週間 |
| 7.10 | **SSH/SFTP対応**（将来） | `[ ]` | リモートファイルシステム（大規模追加）（旧7.8） | ⭐⭐ | TBD |

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

最終更新: 2026-07-18（Phase 7 優先順位の整理。7.3 を Rifle+マジックバイト統合版として昇格、7.2 を後回しに変更）
現在のフェーズ: **Phase 7**（機能開発再開。Phase M1〜M7 全完了、7.1 Leap Navigation は完了済み）
Phase 6: 全タスク完了（6.1 の colors.json 分離のみ後フェーズ送り）
Phase M: 全タスク完了（詳細: [quality_overhaul.md](quality_overhaul.md) の「Phase M 完了サマリ」）
次の作業候補（優先順。実行順の一次情報は上の Phase 7 表の行順）:
1. **7.3 スマート・ファイルオープナー** — Rifle System と旧8.7マジックバイト判定を統合した設計が確定済み（[7.3.smart_file_opener.md](7.3.smart_file_opener.md)）。次に着手する機能
2. **7.6 Undo/Redo** — killer feature、仕様確定済み（7.6.transactional_rollback.md）
3. **7.2 コマンドパレット** — 意図的に後回し。着手前にコンテキスト絞り込み等の設計を詰める

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
