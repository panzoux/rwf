# M5 引き継ぎ — state.rs ハンドラ分割(move-only)

先に `M_handoff_common.md` を読むこと。工数: **大**(3 セッション)。オーケストレータ: **sonnet**。
前提: M4 完了(state.rs 内のダイアログ生成が `new()` 呼びに置換済みで、move 対象が軽くなっている)。

## ゴール

`rwf-lib/src/state.rs`(M4 後の実測行数を S1 で記録: 4741 行)を `rwf-lib/src/state/` ディレクトリへ **move-only** 分割:
- `state/mod.rs` — AppState 定義 + `update_state` dispatch + 再エクスポート(既存の `use crate::state::X` を壊さない)
- `state/handlers/navigation.rs / tab.rs / marking.rs / job.rs / job_management.rs / ui.rs / view.rs / search.rs / viewer.rs / advanced.rs / dialog.rs` — 既存の `handle_*_transition` 関数に対応
- `state/helpers.rs` — 複数ハンドラから呼ばれる共有ヘルパ

## 確定済み設計(再設計禁止)

- **AppState 本体は分割しない**。フィールド追加・削除・型変更は一切禁止(全て move のみ)。
  根拠と再検討トリガーは quality_overhaul.md の M5 節。
- unwrap(state.rs に 4 箇所)は**触らない**。`#![allow(clippy::unwrap_used)]` は分割後、
  unwrap を実際に含むハンドラファイルにのみ付け替える(全ファイルに撒かない)。ratchet リスト更新。
- 挙動保存の機械的確認: 分割前後で `cargo test -p rwf-lib -- --list` の件数一致。
- **並列 haiku 編集は禁止**(可視性調整・use 整理に判断が要る)。調査のみ Explore(haiku)並列可。

## セッション分割

### S1(sonnet + Explore×haiku 並列): マップ作成 + 骨格 + 3 ハンドラ
1. Explore(haiku)2 体並列で読み取り調査:
   (a) 関数→ハンドラ所属マップ(state.rs の全 fn がどの handle_* から呼ばれるか。複数から呼ばれるもの = helpers 行き)
   (b) AppState フィールド所有権マップ(どの handle_* がどのフィールドを読む/書くか)
   結果を本ファイル末尾の「調査結果」欄に貼る(S2 以降と M6/M7 が使う)。
2. `state/` ディレクトリ化 + mod.rs 骨格(この時点で全コード mod.rs のまま、コンパイル緑、1 コミット)。
3. 小さいハンドラ 3 個を handlers/ へ move(1〜2 個ずつコミット)。
- 完了条件: clippy + rwf-lib の `--list` 件数一致 + rwf テスト緑、進捗記入、コミット。

### S2(sonnet 単独): 残りハンドラ move
残り 7〜8 ハンドラを 2〜3 個ずつコミットしながら move。大物(navigation / dialog 系)は単独コミット。
`pub(crate)`/`use` の調整は最小限(公開範囲を広げすぎない)。
セッション末に rwf-lib フルテストをバックグラウンド起動し、待ち時間で S3 の文書タスクに着手してよい。

### S3(sonnet 単独): helpers 整理 + 文書化 + 完了処理
1. 共有ヘルパを helpers.rs へ集約(所属マップ準拠)。
2. `docs/ARCHITECTURE.md` に「AppState フィールド所有権マップ」を追記(S1 調査結果を清書)。
3. 明白に凝集したフィールドグループが調査で見つかった場合のみ、**1 グループ上限**でサブ struct 化
   (該当なしならスキップ。迷ったらやらない — churn 回避が方針)。
4. ratchet リスト更新(state.rs の allow の行き先を記録)。
5. 検証一式全緑 → ROADMAP の M5 を `[x]`、進捗欄を完了に。

## 進捗チェックボックス

- [x] S1-1 調査マップ 2 種(結果は下の欄)
- [x] S1-2 state/ 骨格化コミット(458b066)
- [x] S1-3 ハンドラ move: 3 / 10(実測は 11 ではなく 10 — 下記「設計上の注記」参照)(32055d3)
- [x] S2 ハンドラ move: 10 / 10(job_management/tab: c04ef90, navigation/advanced: df616a2,
      viewer: 24e24ae, job: 102e1da, ui: e3b9fab)。
      dialog.rs への分離は**見送り**(ui.rs 1 本のまま)。理由: `handle_ui_transition` 内の
      ダイアログ関連 match アームはダイアログ以外の UI 系アーム(ChangeUIMode/UpdatePaneHeight/
      ToggleTaskPanel 等)と `self.dialogs` 操作が密に絡み合っており、機械的な行範囲抽出以上の
      判断(どのヘルパーをどちらに渡すか)が必要になる。move-only 原則(判断を増やさない)を優先し
      見送り。実測 10 ハンドラ全て state/handlers/ 配下に move 完了、mod.rs に `fn handle_*` は 0 件。
- [ ] S3-1 helpers.rs 集約(editor_job が唯一の候補 — 調査結果(a)参照)
- [ ] S3-2 ARCHITECTURE.md 所有権マップ
- [ ] S3-3 サブ struct 化(実施 or 見送り理由: ___)
- [ ] S3-4 全検証緑 + ROADMAP 更新

## 設計上の注記(S1 で判明・再設計ではなく実態記録)

handoff 記載の「11 ハンドラ(...dialog.rs)」だが、実コードには `handle_dialog_transition` という独立関数は存在しない。
ダイアログ系 Transition(ShowDialog/CloseDialog/ConfirmDialog/CancelDialog/ShowCustomFunctionsDialog/
ShowDriveChangeDialog/ShowRegisteredFolderDialog/ShowJumpToPathDialog/ShowJumpToFileDialog/
ShowSplitJoinDialog/ShowPatternRenameDialog)は全て `handle_ui_transition`(1668-2297 行、630 行)内に
同居している。実測ハンドラ数は **10**:
navigation(180行) / tab(128行) / marking(44行・move済) / job(812行) / ui(630行・dialog混在) /
view(79行・move済) / search(30行・move済) / viewer(299行) / advanced(233行) / job_management(117行)。

S2 で `handlers/ui.rs` へ move する際、`handle_ui_transition` 内のダイアログ関連 match アーム群を
`handlers/dialog.rs` 側の別関数（例: `handle_dialog_transition`）に切り出し、`handle_ui_transition` から
呼び出す形にすれば handoff の意図(dialog.rs 分離)を move-only の範囲で満たせる。単純な関数抽出であり
ロジック変更ではない。S2 実施者はこの分割を行うか、機械的な負荷を避けてui.rs 1本のままにするかを
判断して進捗欄に記録すること(どちらも move-only の範囲内)。

## 調査結果(S1 で記入)

### (a) 関数→ハンドラ所属マップ

ヘルパー / 自由関数 | 呼び出し元(ハンドラ名 or SHARED) | 備考
---|---|---
`resolve_editor` | (private helper, `editor_job` からのみ) | 単独呼び出し
`editor_job` | SHARED — job, ui | CompleteJob(job) / EditConfigFile・OpenWithEditor(ui)
`save_viewer_to_current_tab` | tab | NextTab/PrevTab/SwitchTab
`restore_viewer_from_tab` | SHARED — tab, job(CloseTab 経由で is_active 分岐、実体は tab 系ロジック) | NextTab/PrevTab/SwitchTab/CloseTab
`start_viewer_search_background` | viewer | ViewerStartSearch/ViewerToggleCaseSensitive
`current_tab` / `current_tab_mut` / `active_pane` / `active_pane_mut` | 全ハンドラで使用(ubiquitous) | helpers.rs 行きではなく AppState に残す想定(pub メソッドのため各ファイルから直接呼べる)
`opposite_pane` | SHARED — marking, ui(Copy/Move/Delete), advanced(SyncPanes) | pub メソッドのため分割後も直接呼べる、helpers.rs 移動は不要
`unmark_all_panes` | job(CompleteJob 後の Move/Delete 完了時) | pub メソッドのため直接呼べる
`collect_jump_path_fast_candidates` / `collect_jump_file_fast_candidates` / `get_share_root_from_location` | ui(ShowJumpToPathDialog/ShowJumpToFileDialog/ShowDriveChangeDialog) | free fn。ui.rs(または dialog.rs)へ move

補足: `current_tab`/`active_pane`等の pub ヘルパーは AppState の impl として mod.rs に残せば
どの handlers/*.rs からも `self.current_tab_mut()` の形でそのまま呼べるため、move 時の書き換え不要。
真に helpers.rs へ切り出すべきは非 pub かつ複数ハンドラ共有の `editor_job` のみ。

### (b) AppState フィールド所有権マップ

フィールド | 主な所有ハンドラ | 備考
---|---|---
`tabs` | tab(作成/削除/切替) | job/navigation/viewer/advanced からも参照(cross-cutting、分割不可)
`jobs` | job | tab/viewer は cleanup(request_cancel)のみ
`background_jobs` | job | tab は CloseTab の cleanup のみ
`search` | search | ui(ConfirmDialog)/dispatch(UpdateDialogInput)が参照
`ui`(UIState) | cross-cutting(navigation/tab/job/view/viewer/advanced/dispatch 全体で参照) | 単一所有者なし、mod.rs 直下の共有フィールドとして扱う
`dialogs` | ui | job(CompleteJob failure)がエラーダイアログ push
`registered_folders` | ui | single-owner
`cache` | job(書込) / navigation・advanced(読取) | job が主所有
`navigation_cache` | navigation | single-owner
`viewer`/`viewer_job_id`/`viewer_search_job_id`/`viewer_search_input`/`viewer_command_input` | viewer | tab の save/restore ヘルパーが橋渡し
`log_manager` | ui(SaveLog) | 他は初期化のみ
`config` | dispatch(ReloadConfig/UpdateConfig) | navigation/job/viewer/ui が値を読むのみ
`last_tab_created` | tab | single-owner(CreateTab debounce)
`extension_associations`/`custom_functions`/`config_load_results` | dispatch(ReloadConfig) | ui は custom_functions を読むのみ
`pending_confirmation_logs`/`confirmation_needs_keybinding_reload`/`pending_custom_function_input`/`suppress_next_dialog_pop` | どのハンドラからも未使用 | app.rs 側で読み書き(rwf-bin 側の統合レイヤー)。state.rs 内では未参照フィールド
`leap` | dispatch(Leap* transitions) | single-owner、ただし update_state 内の match アーム直書き(handle_* を経由しない)

結論: S3-3(サブ struct 化)の有力候補は無し(`ui`/`tabs`/`config` は cross-cutting で単純化に不向き、
それ以外は既に単一所有)。見送り前提で進める。

## セッション開始プロンプト(コピペ用)

```
plan/M_handoff_common.md と plan/M5_handoff.md を読み、M5 のセッション S<N> を実施してください。
進捗は M5_handoff.md のチェックボックスが正です。move-only 厳守(ロジック変更・unwrap 修正は禁止)。
完了条件を満たしたらチェックを更新してコミットしてください。
```
