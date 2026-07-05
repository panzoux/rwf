# M5 引き継ぎ — state.rs ハンドラ分割(move-only)

先に `M_handoff_common.md` を読むこと。工数: **大**(3 セッション)。オーケストレータ: **sonnet**。
前提: M4 完了(state.rs 内のダイアログ生成が `new()` 呼びに置換済みで、move 対象が軽くなっている)。

## ゴール

`rwf-lib/src/state.rs`(M4 後の実測行数を S1 で記録: ___ 行)を `rwf-lib/src/state/` ディレクトリへ **move-only** 分割:
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

- [ ] S1-1 調査マップ 2 種(結果は下の欄)
- [ ] S1-2 state/ 骨格化コミット
- [ ] S1-3 ハンドラ move: 3 / 11
- [ ] S2 ハンドラ move: 11 / 11
- [ ] S3-1 helpers.rs 集約
- [ ] S3-2 ARCHITECTURE.md 所有権マップ
- [ ] S3-3 サブ struct 化(実施 or 見送り理由: ___)
- [ ] S3-4 全検証緑 + ROADMAP 更新

## 調査結果(S1 で記入)

```
(関数→ハンドラ所属マップ / フィールド所有権マップを S1 で貼る)
```

## セッション開始プロンプト(コピペ用)

```
plan/M_handoff_common.md と plan/M5_handoff.md を読み、M5 のセッション S<N> を実施してください。
進捗は M5_handoff.md のチェックボックスが正です。move-only 厳守(ロジック変更・unwrap 修正は禁止)。
完了条件を満たしたらチェックを更新してコミットしてください。
```
