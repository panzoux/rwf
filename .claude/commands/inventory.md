# /project:inventory — 開発環境 定期棚卸

rwf の開発環境を棚卸し、「調査→レポート→承認後に実行」の順で進めること。調査段階では何も変更しない。

## 調査項目

1. **テストベースライン実測**
   - `cargo test -p rwf --no-run` でコンパイル可否を確認（rwf-bin テストの破損検出）
   - `cargo test -p rwf-lib -- --test-threads=1` をバックグラウンドで実行し、失敗数と失敗テスト名を
     `.claude/CLAUDE.local.md` の「Baseline Failures」と突き合わせる。乖離があれば両方更新を提案
   - `cargo clippy --all-targets` の warning 数を前回値と比較

2. **ROADMAP と git 履歴の同期**
   - `git log --oneline` を ROADMAP.md 最終更新日以降分読み、完了済みなのに未マークの項目・
     ROADMAP外で実装された機能を洗い出す
   - plan/ 配下の詳細mdファイル名と ROADMAP のフェーズ番号・リンクの整合を確認

3. **作業ツリーの残骸**
   - `git status --short` の未追跡/変更ファイルを分類（バックアップ、proptest回帰、一時ファイル）
   - proptest-regressions に新規エントリがあれば、対象プロパティテストを読んで
     「実装バグ / テストバグ / 正当な回帰」を判定

4. **設定の乖離**
   - `%APPDATA%\rwf\` の実設定と `rwf-lib/resources/` のデフォルトを突き合わせ
     （キー衝突、マクロ衝突 = bare `$VAR` で P/O/L/R/F/W/E/M 開始、欠落ファイル）
   - `.claude/settings.json` / `settings.local.json` に一度きりの許可残骸が溜まっていないか

5. **プロンプト/メモリの鮮度**
   - `.claude/CLAUDE.local.md` の事実記述（テストベースライン、現在フェーズ）が実測と一致するか
   - メモリファイル（`~/.claude/projects/.../memory/`）の description と本文の鮮度、
     MEMORY.md インデックスとの一致、`[[リンク]]` 切れ

## 進め方

1. 調査結果を箇条書きレポートで提示（変更なし）
2. 優先順位付き改善プランを提示し承認を待つ
3. 承認項目のみ実行。削除・ROADMAP/CLAUDE.md 書き換え・git 操作は個別確認
4. コード修正系はコスト削減のため Sonnet subagent への委任を検討する
