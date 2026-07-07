# Phase M — 品質整備フェーズ 詳細計画

作成: 2026-07-05 / 状態: M1〜M3 完了・M4 未着手 / 概要は [ROADMAP.md](ROADMAP.md) の Phase M ブロック参照

## 背景と目的

rwf は twf プロトタイプを基に複数の AI アシスタント（kilo → antigravity → qwen → gemini → claude）を
渡り歩いて開発されており、アーキテクチャの骨格（Transition ベース純粋ステートマシン、
JobManager/WorkerPool による非同期 I/O 分離、rwf-lib/rwf-bin 分離）は健全だが、
典型的な「AI 渡り歩き」の痕跡が蓄積している。

**本来の問いは「コードをどうきれいにするか」ではなく
「今後も AI 主導で開発を続けながら品質が劣化し続けない仕組みをどう作るか」。**
一回きりの大掃除では次の AI セッションが再び ad-hoc コードを生成して数ヶ月で元に戻る。

**回答 = 3 層アプローチ:**
1. **再利用部品の整備** — 正しい書き方を「一番楽な書き方」にする（共通ヘルパ・fixture があれば AI は自然にそれを使う）
2. **機械的ガードレール** — lint / fmt / CI / CLAUDE.md レシピで、それ以外の書き方を機械的に弾く
3. **構造の段階的分割** — 巨大ファイル・god-struct をテストを盾に順次解消

## 調査で確認した問題（2026-07-05 時点）

| 問題 | 場所 | 規模 |
|---|---|---|
| god-file + unwrap/clone 多用 | `rwf-lib/src/state.rs` | 3,886 行、unwrap 273、AppState 13+ public フィールド |
| render 関数コピペ 18 個 | `rwf-bin/src/ui/dialog/mod.rs` | 4,434 行、スタイル定義インライン 10+ 箇所重複 |
| UI 状態とデータ混在 | `rwf-lib/src/model/dialog.rs` | 2,743 行、ダイアログ生成に 5+ フィールド手動セット |
| テスト fixture 重複 | `rwf-lib/src/*_tests.rs` 約 40 ファイル | 推定 2,000 行超の重複セットアップ |
| 非テストコード unwrap | ワークスペース全体 | grep 概算 455 箇所 → **clippy 実測 35 箇所・9 モジュール**（M1 で確定。差分はテストコード） |
| clone 過多 | rwf-lib | 799 箇所（ホットパス性能リスク） |
| ガードレール欠如 | ルート | lint 属性なし、rustfmt.toml/clippy.toml なし、CI に fmt チェックなし |
| レシピ不在 | docs/ | 「ダイアログ/Transition の追加方法」なし → AI がコピペに走る根本原因 |
| UI 層ほぼ未テスト | rwf-bin | 17 ファイルにテストなし、スナップショットテストなし |

## 全体方針

- **機能開発は凍結**（Phase 7 残タスク・Phase 8+ は M 完了後。ROADMAP 上も Phase M は Phase 7 より前に位置づけ）。全フェーズは挙動保存が原則。
  挙動変更を伴う修正（archive.rs の TODO 等）は M7 に隔離し単独コミット。
- **順序の根拠**: ガードレールと共有部品を先に敷き、後続の大規模分割を機械的検証で保護する。
  unwrap/clone 監査は構造分割**後**（分割前に監査すると同じ行を二度触る）。
- **各フェーズは独立着地**: フェーズ末で `cargo fmt --all -- --check` +
  `cargo clippy --all-targets -- -D warnings` + `cargo test -p rwf -- --test-threads=1` +
  `cargo test -p rwf-lib -- --test-threads=1` 全緑 + コミット。検証は `/project:check` に一本化（M1 で fmt 追加）。
- **CI 制約**: clippy は `-D warnings` で走るため warn レベル lint も実質 deny。
  unwrap_used の段階導入は「モジュール単位 `#![allow]` → 順次撤去（ratchet）」方式のみ有効。
- `volume_info.rs` に Win32 API の unsafe が 6 箇所あるため `forbid(unsafe_code)` は不可。
  `deny` + 当該モジュールのみ `#[allow]` + 各 unsafe ブロックに `// SAFETY:` コメント。

---

## Phase M1 — ガードレール導入（fmt / lint / CI）

**ゴール**: 以降の全リファクタが自動検出網の中で行われる状態を作る。コード実体の変更はフォーマットのみ。

1. `rustfmt.toml` 追加（ほぼデフォルト、edition 明記のみ。カスタム設定は AI 生成コードとの摩擦を増やすだけ）
   → `cargo fmt --all` を**独立コミット** + `.git-blame-ignore-revs` 追加で blame 汚染回避。
2. `clippy.toml` 追加: `allow-unwrap-in-tests = true` / `allow-expect-in-tests = true`
   （テストコードを 455 サイト問題から除外）。
3. ルート `Cargo.toml` に `[workspace.lints]`、両クレートに `[lints] workspace = true`:
   - `unsafe_code = "deny"`（volume_info.rs のみ scoped allow + SAFETY コメント）
   - `clippy::unwrap_used = "deny"` — 現時点で違反のあるモジュール先頭に
     `#![allow(clippy::unwrap_used)] // TODO(M6): ratchet` を機械挿入。
     **新規・修正済みモジュールは即座に保護される**のが狙い。
   - `clippy::expect_used` は導入**しない**（M6 の着地点が「メッセージ付き expect」のため）。
     `missing_docs` も現段階では入れない（M7 で判断）。
4. `.github/workflows/ci.yml` の lint ジョブに `cargo fmt --all -- --check` 追加。
5. 本ファイル末尾に「allow ratchet リスト」（M1 時点で allow を持つモジュール一覧）を記録 → M6 の burn-down リスト。

**規模**: 小（1 セッション） / **実行**: 本体セッション sonnet、subagent 不要（fmt→lint の依存があり並列化しない）
**リスク**: fmt 後に clippy 新規警告（低）。fmt と lint のコミットを分けて切り戻し容易に。

## Phase M2 — 共有部品とドキュメント基盤

**ゴール**: (a) 後続分割の道具となる共有テスト fixture・描画ヘルパ、(b) AI がコピペに走る根本原因（レシピ不在）を塞ぐドキュメント基盤。2 ワークストリームは独立・並列可。

**A: 共有部品**
1. `rwf-lib/src/test_utils.rs`（`#[cfg(test)] pub mod test_utils;`）: TempDir ハーネス、
   `AppStateBuilder`（既定値+差分指定）、FileEntry ファクトリ、ダイアログ起動ヘルパ。
   まず既存テスト 3〜4 ファイルを移行して API を実証。
2. 残り 〜35 テストファイルの fixture 移行（推定 2,000 行削減）。
   **「セットアップに意図的差分があるものは移行せず残す」ルール厳守**。移行前後で
   `cargo test -- --list` の件数一致を確認。
3. `rwf-bin/src/ui/dialog/common.rs`: スタイル定数化（`Style::default().fg(Black).bg(Gray)` 等
   インライン 10+ 箇所）、centered_rect / タイトル付き Block / ボタン行レンダラ共通関数。
   既存 `ui/colors.rs` との整合を確認して配置決定。
4. rwf-bin ダイアログテスト用ヘルパ（13 個の可変変数を束ねる struct + コンストラクタ）。
   既存 50+ テストの移行は M3 で実施（M3 でどうせ触るため）。

**B: ドキュメント**
5. ルート `CLAUDE.md` 新設: `.claude/CLAUDE.local.md` の共有可能部分を昇格 + ビルド/テストコマンド +
   品質規約（unwrap 禁止・test_utils 使用必須・ダイアログは common.rs 経由）+ レシピへのポインタ。
6. `docs/ARCHITECTURE.md`: Transition ステートマシン、JobManager/WorkerPool、lib/bin 境界、
   AppState 責務境界（M5 の布石）。DEVELOPER_GUIDE.md と重複させず参照。
7. `docs/TESTING.md`: test_utils の使い方、`--test-threads=1` の理由（fs 競合）、PROPTEST_CASES 方針。
8. stale 参照修正: USER_GUIDE.md の `two-pane-fm` → `rwf` ほか grep 掃除。
9. `docs/recipes/`（add-a-dialog.md / add-a-transition.md）は**ドラフトのみ**（M3/M4 後に構造が変わるため、確定は M7）。

**規模**: 中（2〜3 セッション）
**実行**: fixture 移行 = 並列 general-purpose × haiku（5〜7 ファイル/エージェント、test_utils API 例と
「テスト件数を変えない」制約を明示）。API 設計とドキュメント = sonnet 本体。

> **M2 実施メモ（2026-07-05 完了）**
> - `test_utils.rs`: `test_state()` / `FileEntryBuilder`（`calculated_size`・`modified` 等全フィールド
>   セッター付き）/ `entry`・`entries`・`numbered_entries` / `AppStateBuilder` / `temp_dir`・
>   `state_with_temp_dirs` / `open_dialog`・`current_dialog`。
> - fixture 移行 40 ファイル（パイロット 4 + 並列 haiku）。テスト件数 1043 で前後一致。
>   **意図的未移行**: `config_display/integration/keybindings/launch`・`concurrent_operations`・
>   `multi_language_help`・`log_management`（カスタム AppConfig）、`sevenz`・`tar`・
>   `archive_format_recognition`（実 FS/アーカイブセットアップ）、`help_viewer`（対象なし）。
> - rwf-bin: `ui/dialog/common.rs`（`DIALOG_*` 定数 + `titled_block`。ratatui 0.29 の const Style を利用）
>   を新設し frame.rs と mod.rs の数箇所へ最小適用（全面適用は M3）。既存 `frame.rs` が
>   centered_rect/枠/ボタン行を既に提供していたため、common.rs はスタイル定数に限定し重複を回避。
>   `test_support::ConflictInputHarness`（13 可変変数 + conflicts/history を束ねる）新設、1 テスト移行済み。
> - ドキュメント: ルート `CLAUDE.md` / `docs/ARCHITECTURE.md` / `docs/TESTING.md` /
>   `docs/recipes/`（ドラフト 2 本）/ stale 参照修正（USER_GUIDE の two-pane-fm→rwf、
>   DEVELOPER_GUIDE のディレクトリツリーを実ワークスペース構成に更新）。
> - **並列 haiku 運用の教訓**（M3 のスナップショット量産で再利用すること）:
>   (1) セッション上限でエージェントが途中死すると半端編集が残る — 「1 ファイルずつ一貫状態で
>   完結させる」指示を必須にする。(2) `entry()`（setter なし）と `FileEntryBuilder` の混同、
>   `calculated_size: Some(..)` の脱落が実際に発生 — coordinator 側で
>   `git diff` の非デフォルト値監査（`calculated_size: Some|marked: true|UNIX_EPOCH` 等の
>   削除行と追加行の突合）を必ず実施する。

## Phase M3 — dialog/mod.rs 分割（4,434 行 → ファイル群）+ スナップショットテスト導入

**ゴール**: UI 最大のモノリスを分割。**分割前にスナップショットで挙動を固定**するのが要点。

1. dev-dependency に `insta` 追加（単独コミット）。ratatui `TestBackend`（80×24 / 120×40 の 2 サイズ固定）で
   18 ダイアログ全種 × 代表状態 2〜4 のスナップショットテスト作成。**このコミットが分割の安全網**。
2. `rwf-bin/src/ui/dialog/` を分割: `mod.rs`（dispatch match のみ 〜100 行）+
   `file_conflict.rs` / `sort.rs` / `file_mask.rs` / `wildcard_mark.rs` / `simple_rename.rs` /
   `history.rs` / `drive_selection.rs` / `context_menu.rs` / `jump_to_path.rs` / `jump_to_file.rs` /
   `file_info.rs` / `pattern_rename.rs` / `help.rs` / `job_manager.rs` / `compression.rs` 等。
   各ファイルで M2 の common.rs を適用し重複削減。
3. 既存 50+ ダイアログテストを M2 ヘルパへ移行し各ダイアログファイルへ同居。
4. スナップショット差分ゼロ確認（意図的差分は原則出ない — 出たら実装ミス）。
5. `docs/TESTING.md` に insta 運用（`cargo insta review`）を追記。

**規模**: 大（3〜4 セッション）。ダイアログ単位で完全独立。
**実行**: スナップショット量産と切り出しは並列 general-purpose × haiku（見本 1 本を sonnet が先に作成、
最初の 2 ダイアログも sonnet でパターン確立 → 残りを haiku 展開）。mod.rs の `mod` 宣言は
並列展開**前**に全部先行コミットしてコンフリクト面を消す。統合は sonnet。

> **M3 実施メモ（2026-07-05 完了）**
> - 安全網: `snapshot_tests/`（ハーネス + 全 29 バリアント × 80×24/120×40 = 94 テスト/188 snap）。
>   Buffer の Debug 出力（テキスト + スタイル run）をそのまま snapshot。決定性は 3 回連続実行で検証。
>   タイムゾーン/揮発対策: insta filters で `YYYY-MM-DD HH:MM:SS`・`HH:MM:SS`・UUID を redact。
>   JumpTo 系は Transition 経由だと実 FS を走査するため content 直組み。job_manager は
>   HashMap 順序が不定なためジョブ 1 件まで。
> - 分割: move-only で 17 ファイル（batch1: リスト/入力系 10、batch2: jump×2/file_info/
>   pattern_rename/help、batch3: file_conflict 一式 + conflict_tests 同居、batch4: basic.rs
>   （render_dialog_content/handle_content_input）+ confirm.rs（process_dialog_confirmation 系））。
>   mod.rs 5,409→2,024 行。**残り**: render_dialog のサイズ計算+dispatch と handle_dialog_input の
>   inline 腕 — 腕本体の関数化は現状 15 引数関数を量産するだけなので、M4 のバリアント struct 化後に
>   `handle_input(&mut FooDialog, key)` 形式で実施する（計画の「mod.rs 〜100 行」はそこで達成）。
> - common.rs 適用: `Style::default().fg(..).bg(..)` 81 箇所を `DIALOG_*` 定数へ機械置換
>   （regex、完全修飾パス）。snap 差分ゼロで挙動保存を証明。
> - conflict 入力テスト 6 本を `ConflictInputHarness` へ移行（13 変数宣言を撤去）。
> - 抽出は PowerShell スクリプト（brace カウント + LF 保持 WriteAllText）で機械化。
>   char リテラル `'"'` で brace カウントが狂う既知の穴あり — テスト mod は EOF 起点で切り出した。
>   並列 haiku は今回もセッション上限で 4/5 バッチ途中死 → スナップショット 7 ファイルは本体で作成。

## Phase M4 — model/dialog.rs 分割と UI 状態/ビジネスデータ分離

**ゴール**: DialogContent の「生成に 5+ フィールド」問題を解消し、ダイアログ追加コストを一定化。

1. `rwf-lib/src/model/dialog/` ディレクトリ化: ダイアログごとの struct を個別ファイルへ。
   `DialogContent` enum 自体は `mod.rs` に残す（enum は Transition dispatch と噛み合っており壊す価値がない）。
2. 各バリアントのフィールド群を専用 struct に畳む（`DialogContent::Sort(SortDialog)` 形式）。
   カーソル/スクロール/ボタンフォーカス等の共通 UI 状態は `DialogUiState` として各 struct に埋め込む。
   完全ジェネリック化（`Dialog<T>`）は**やらない** — 18 種の UI 状態は微妙に異なり、
   無理な抽象化は AI 生成コードで一番壊れやすい。
3. 各ダイアログに `new()` コンストラクタ（必須引数のみ、UI 状態はデフォルト初期化）を用意し、
   state.rs 側の手動フィールドセットを全置換。
   **バリアント単位で「struct 化 + 呼び出し側置換」を 1 コミット**（逐次着地、全バリアント一斉にしない）。
4. `docs/DIALOG_DESIGN_SPEC.md` を新構造に更新。

**規模**: 中〜大（2〜3 セッション）
**実行**: DialogUiState の切り方・コンストラクタ規約は opus/sonnet で先に確定 →
バリアント畳み込みをダイアログ単位で並列 haiku。M5 より先に実施（逆順だと分割後の handlers
全部で同じ置換をする羽目になる）。

## Phase M5 — state.rs 分割と AppState 境界整理

**ゴール**: 3,886 行の state.rs をハンドラ単位モジュールへ move-only 分割。**AppState 本体は分割しない**。

1. `rwf-lib/src/state/` ディレクトリ化: `mod.rs`（AppState 定義 + `update_state` dispatch）+
   `handlers/`（navigation / dialog / job / marking / viewer / search / tab / config 等、
   既存 〜10 の `handle_*_transition` に対応）。共有ヘルパは `helpers.rs`。
2. unwrap 273 箇所は**触らない**（移動のみ）。`#![allow]` は分割後の各ハンドラファイルへ引き継ぎ、
   M6 の ratchet 単位を細かくする — これが M5 を M6 より先にやる理由。
3. AppState フィールドは現状維持。`docs/ARCHITECTURE.md` に「フィールド所有権マップ」
   （どのハンドラがどのフィールドを読む/書くか）を追記。
   明白に凝集していて呼び出し側変更が小さいグループ（候補: ダイアログ関連）が見つかった場合のみ、
   **1 グループ上限**でサブ struct 化を許可。

**AppState 分割見送りの根拠**: 1,094 テストの多くが `state.field` を直接参照しており、
サブ struct 化は数千行の機械的 churn を生むが、Transition dispatch が既に境界を提供しているため
安全性の増分が小さい。実害（どこから触られるか不明）はハンドラ分割 + 所有権マップで解消。
新フィールドの規律は CLAUDE.md 規約（「新フィールドは所有ハンドラを明記」）で担保。
**再検討トリガー**: フィールド 20 超、または複数ハンドラの同一フィールド書き込み競合が M6 監査で発覚した場合。

**検証**: move-only の機械的確認として分割前後で `cargo test -p rwf-lib -- --list` 件数一致。
**規模**: 大（2〜3 セッション） / **実行**: sonnet 主導・逐次（可視性調整・use 整理に判断が要るため
haiku 並列不向き）。事前に Explore × haiku で「関数→ハンドラ所属マップ」を並列作成。
ハンドラ 2〜3 個ずつコミット。他フェーズと並行させない。

## Phase M6 — unwrap/expect 監査 + clone 監査（ratchet 完了）

**ゴール**: M1 の `#[allow(clippy::unwrap_used)]` を全撤去し、非テストコードの unwrap ゼロへ。

1. **棚卸し**: Explore × haiku（モジュール単位並列）で全 unwrap を 3 分類:
   - (a) 不変条件により infallible → `expect("なぜ infallible かを書いたメッセージ")` へ機械変換
   - (b) 実際に失敗しうる → エラー伝播（既存のエラー Transition / エラー経路へ）
   - (c) lock poisoning 等「落ちてよい」系 → expect + メッセージ
2. **修正**: (a)(c) はモジュール単位で並列 haiku、(b) は sonnet が個別対応。
   モジュール完了時に `#![allow]` 削除（= ratchet）。1 モジュール = 1 コミット。
   expect メッセージ規約（「なぜ infallible か」を書く。無意味な文言禁止）をエージェント指示に含める。
3. **clone 監査**: 799 全部は潰さない。対象を絞る:
   (i) `Vec<FileEntry>` 等 O(エントリ数) の clone、(ii) キー入力毎に走るホットパス。
   Explore × haiku で候補抽出 → sonnet が Arc 化 / 借用化 / `mem::take` を判断。
   **小さい struct の clone は許容**と CLAUDE.md に明記（clippy の clone 系 lint は導入しない —
   false positive が AI の過剰修正を誘発する）。

**規模**: 大（3〜5 セッション）
**リスク**: (b) は panic → エラー表示の挙動変更を含む。「品質修正」として許容するが、
変更点をフェーズサマリに列挙すること。

## Phase M7 — 仕上げ（レシピ確定・rustdoc・残タスク・凍結解除）

1. `docs/recipes/add-a-dialog.md` / `add-a-transition.md` を最終構造で確定。
   チェックリスト形式（新ダイアログ = model/dialog/xxx.rs + ui/dialog/xxx.rs +
   handlers/dialog.rs の match 腕 + スナップショットテスト）。CLAUDE.md からリンク。
2. `backend/` `job/` `model/` の公開 API に rustdoc（haiku 並列下書き → sonnet レビュー)。
   `#![warn(missing_docs)]` 導入はここで判断（全公開項目カバー後のみ）。
3. `rwf-lib/src/backend/archive.rs:222` の TODO（タイムスタンプ処理）修正 —
   **本計画唯一の挙動変更**。単独コミット + テスト追加。
4. rwf-bin 未テスト UI 17 ファイルへ最低限の TestBackend スモーク/スナップショット
   （panes / task_panel / viewer / tab_bar 優先。「panic しない + 代表状態」で十分、全網羅不要）。
5. ルート直下の `*_SUMMARY.md` / `BUGFIX_*.md` 類を `docs/history/` へ整理（git mv のみ）。
6. ROADMAP.md の Phase M を完了マークし**機能開発凍結解除**を宣言。
   CI 37 分問題は解決せず課題として記録のみ（fs 競合による single-thread 制約が本質。Phase 8+ 検討）。

**規模**: 中（2 セッション） / **実行**: rustdoc・スモークテスト = haiku 並列、TODO 修正・レシピ = sonnet

---

## フェーズ横断の実行規約（Claude Code 運用）

> **2026-07-05 更新**: M4〜M7 は Fable 5 退役（07-07）に伴い opus/sonnet/haiku での実行に引き継ぐ。
> **実行時は下表より [M_handoff_common.md](M_handoff_common.md) + 各 [M4](M4_handoff.md)/[M5](M5_handoff.md)/[M6](M6_handoff.md)/[M7](M7_handoff.md)_handoff.md が優先**
> （設計判断は handoff に確定済みのため opus は原則不要。M4 の並列 haiku は共有ファイル衝突のため禁止に変更、
> セッション分割・途中死対策・M2/M3 の実事故に基づく haiku 運用ルールも handoff 側に記載）。

| 作業種別 | 実行形態 | モデル |
|---|---|---|
| 棚卸し・監査・所属マップ作成 | Explore サブエージェント並列 | haiku |
| fixture 移行・スナップショット量産・バリアント畳み込み・rustdoc 下書き | general-purpose 並列（バッチ分割、見本テンプレート必須添付） | haiku |
| API 設計・共通 struct 設計・エラー伝播・mod.rs 統合 | 本体セッション逐次 | sonnet |
| M4/M5 の分割方針決定・AppState 再検討 | 本体セッション（計画モード） | opus（判断）→ sonnet（実行） |

- 並列サブエージェントには必ず: 対象ファイル明示 / 見本コード / 「テスト件数を減らさない・挙動を変えない」制約 / 完了時検証コマンド を渡す。
- 並列展開前に共有ファイル（mod.rs の宣言等）への変更を先行コミットしコンフリクト面を消す。
- 大きな move/分割フェーズ（M3/M5）は他作業と並行させない。

## allow ratchet リスト（M1 実施時点 = M6 の burn-down リスト）

> M1 で `#![allow(clippy::unwrap_used)]` を挿入したモジュール。M6 で unwrap を解消するたびに
> allow を削除し、ここから消し込む。
>
> **実測値の注記**: 事前調査の「非テスト unwrap 455 箇所」は grep ベースの概算で、テストコードを
> 多く含んでいた。clippy.toml の `allow-unwrap-in-tests` 適用後に deny が実際に検出した
> 非テスト unwrap は **35 箇所・9 モジュール**。M6 の実作業量は当初想定より大幅に小さい。

| モジュール | unwrap 数 | 状態 |
|---|---|---|
| `rwf-lib/src/model/dialog/mod.rs`(M4 で `dialog.rs` をディレクトリ化) | 11 | `[ ]`(全11箇所は `RegisteredFolderManager::expand_env_vars` にあり、struct化した29バリアントのファイルには unwrap なし。M6 で ratchet) |
| `rwf-lib/src/macro_expander.rs` | 7 | `[ ]` |
| `rwf-lib/src/job/job_executor.rs` | 5 | `[ ]` |
| `rwf-lib/src/state/mod.rs`(M5 で `state.rs` をディレクトリ化) | 4 | `[ ]`(4箇所全て `new()`(1箇所)と `start_viewer_search_background()`(3箇所)にあり、両方とも mod.rs に残留。`state/handlers/*.rs` と `state/helpers.rs` へ move した10ハンドラ+editor_job には unwrap 皆無なので allow 不要。M6 で mod.rs のみ ratchet) |
| `rwf-lib/src/logging.rs` | 3 | `[ ]` |
| `rwf-bin/src/ui/viewer.rs` | 2 | `[ ]` |
| `rwf-lib/src/job.rs` | 1 | `[ ]` |
| `rwf-lib/src/model/viewer.rs` | 1 | `[ ]` |
| `rwf-lib/src/pattern_rename.rs` | 1 | `[ ]` |

unsafe_code は `rwf-lib/src/volume_info.rs` のみ `#![allow(unsafe_code)]`（Win32 API、
全 4 ブロックに SAFETY コメント付与済み）。これは恒久的な scoped allow であり ratchet 対象外。
