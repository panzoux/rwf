# Phase 6.6 — Custom Function Menu Dialog

**作成**: 2026-06-08  
**参照**: `specs/twf/TWF/menu_*.json`, `specs/twf/CUSTOM_FUNCTIONS.md`

---

## 目的

`custom_functions.json` でメニュー型のカスタム関数（`Menu` フィールド）を選択した際に、
対応する `menu_xxx.json` の内容をダイアログ表示して実行できるようにする。

---

## TWF の実装仕様（参照元から判明した事実）

### menu_xxx.json のフォーマット

```json
{
  "Version": "1.0",
  "Menus": [
    { "Name": "Open in Notepad",        "Action": "Open in Notepad" },
    { "Name": "Copy to Other Pane",     "Action": "Copy to Other Pane" },
    { "Name": "-----",                   "Action": "" },
    { "Name": "Delete File",            "Action": "DeleteFile" },
    { "Name": "reload config",          "Action": "ReloadConfiguration" }
  ]
}
```

### `Action` フィールドの解決ルール（重要）

`Action` フィールドは **カスタム関数名とビルトインアクション名の両方**に使われる。
解決順序：
1. ビルトインアクション名として一致するか確認（例: `"DeleteFile"`, `"ReloadConfiguration"`, `"ViewFileAsText"`）
2. 一致しない場合は `custom_functions.json` のカスタム関数名として検索して実行

`"Function"` フィールドはドキュメントに記載があるが、実ファイルでは使われておらず、
`"Action"` のみ使用する。

### セパレータ

```json
{ "Name": "-----", "Action": "" }
```

`Name` が `"-----"` で始まるか、`Action` が空文字列のアイテムはセパレータ。選択不可。

### ネスト制限

menu_xxx.json から別の menu_xxx.json を参照することは**サポートしない**（TWF 仕様）。
階層は常に：
```
custom_functions.json の Function（Menu型）
  └─ menu_xxx.json の MenuItem（Action フィールドでカスタム関数 or ビルトイン実行）
```
2段階まで。menu ファイルからさらに menu を開くことは不可。

### ナビゲーション

| キー | 動作 |
|------|------|
| Up / Down | セパレータをスキップして前後の選択可能アイテムへ |
| 文字キー | その文字で始まる次のアイテムへジャンプ（インクリメンタル検索ではない） |
| Enter | 選択アイテムを実行 |
| Esc | メニューを閉じる（カスタム関数ダイアログには戻らない） |

---

## 現状との差異（修正が必要な点）

### 1. `MenuContent` の型が間違っている

現在:
```rust
pub enum MenuContent {
    Inline(Vec<CustomFunction>),  // ← menu item ではなく CustomFunction を格納している
    File(String),
}
```

`menu_xxx.json` のアイテムは `CustomFunction` ではなく `MenuItem`（Name + Action）構造。
修正: `Inline(Vec<MenuItem>)` に変更。

### 2. `resolve_menu_files` が間違ったパーサーを使っている

現在、`menu_xxx.json` を `CustomFunction[]` としてパースしようとしているため失敗する。
修正: `MenuFile { menus: Vec<MenuItem> }` としてパースするように変更。

### 3. ダイアログが menu 型エントリを無視している

現在、カスタム関数ダイアログで Enter を押すと `is_menu()` に関係なく即コマンド実行を試みる。
修正: `is_menu()` の場合はメニューダイアログを開く遷移へ変更。

---

## 実装計画

### Step 1 — データモデル (`rwf-lib/src/model/dialog.rs`)

#### 1a. `MenuItem` struct を追加

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuItem {
    pub name: String,
    /// Custom function name or built-in action name.
    /// Empty string = separator.
    #[serde(default)]
    pub action: String,
}

impl MenuItem {
    pub fn is_separator(&self) -> bool {
        self.name.starts_with("-----") || self.action.is_empty()
    }
    pub fn is_selectable(&self) -> bool { !self.is_separator() }
}
```

#### 1b. `MenuFile` struct を追加

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MenuFile {
    #[serde(default)]
    pub version: String,
    pub menus: Vec<MenuItem>,
}
```

#### 1c. `MenuContent` を変更

```rust
pub enum MenuContent {
    Items(Vec<MenuItem>),  // 解決済みアイテムリスト
    File(String),          // ロード前のファイル参照（中間状態）
}
```

`is_menu()` は `Some(_)` で判定（変更なし）。
`menu_items()` は `Items(items) → items` を返すように更新。

#### 1d. `resolve_menu_files` を修正

`File(filename)` の場合、`MenuFile` としてパースして `Items(menus)` へ変換する。
menu ファイル内からの再帰解決は不要（TWF仕様でネスト禁止）。

inline の `Items(Vec<MenuItem>)` はそのまま保持。

### Step 2 — `DialogContent` に menu ダイアログを追加 (`rwf-lib/src/model/dialog.rs`)

```rust
DialogContent::CustomFunctionMenu {
    items: Vec<MenuItem>,
    selected_index: usize,
    // カスタム関数リストへの参照は不要（Esc で全閉じ）
}
```

`Dialog::custom_function_menu(title: String, items: Vec<MenuItem>)` ファクトリも追加。

### Step 3 — カスタム関数ダイアログの Enter 処理を変更 (`rwf-bin/src/ui/dialog/mod.rs`)

現在 Enter → `DialogAction::Confirm` のみ。

変更:
```rust
KeyCode::Enter => {
    if let Some(func) = selected_function() {
        if func.is_menu() {
            return DialogAction::OpenMenu(func.menu_items().to_vec());
        } else {
            return DialogAction::Confirm;
        }
    }
}
```

`DialogAction::OpenMenu(items)` を app.rs で受け取り、`Dialog::custom_function_menu(...)` を
ダイアログスタックにプッシュする。

### Step 4 — メニューダイアログの描画 (`rwf-bin/src/ui/dialog/mod.rs`)

`render_custom_function_menu(frame, area, items, selected_index)`

- セパレータは "────────" などの罫線表示（または "---" のままでもよい）
- 選択アイテムはハイライト、セパレータはハイライト不可
- ヒント行: `[Enter] Execute  [Esc] Close`
- 文字キー表示: フィルタではなくジャンプインジケータ

### Step 5 — メニューダイアログのキー処理

```rust
KeyCode::Up   → 前の selectable アイテムへ（セパレータスキップ）
KeyCode::Down → 次の selectable アイテムへ（セパレータスキップ）
KeyCode::Char(c) if !modifier → 
    その文字で始まる次の selectable アイテムへジャンプ
KeyCode::Enter → DialogAction::ExecuteMenuItem
KeyCode::Esc   → DialogAction::Cancel（全閉じ）
```

### Step 6 — `Action` フィールドの解決 (`rwf-lib/src/input/mod.rs` or `state.rs`)

`ExecuteMenuItem(action_name: String)` 遷移を追加。

解決テーブル（ビルトインアクション名 → `input::Action`）:

| TWF Action 名 | rwf Action |
|---------------|-----------|
| `"DeleteFile"` / `"Delete"` | `Action::Delete` |
| `"MoveFile"` / `"Move"` | `Action::Move` |
| `"ViewFileAsText"` | `Action::OpenTextViewer` |
| `"ViewFileAsHex"` | `Action::OpenHexViewer` |
| `"ReloadConfiguration"` | `Action::ReloadConfig` |
| `"LaunchConfigurationProgram"` | `Action::LaunchConfigurationProgram` |

上記に一致しない場合 → `state.custom_functions` から名前で検索してコマンド実行。

### Step 7 — config_load_results への menu ファイル追加

現在 `context_menu.json` のみバリデーション。
起動時に `custom_functions.json` と同ディレクトリの `menu_*.json` も JSON バリデーションして
`[OK]`/`[NG]`/`[Skipped]` を `config_load_results` に追加する。

---

## 変更対象ファイル

| ファイル | 変更内容 |
|----------|----------|
| `rwf-lib/src/model/dialog.rs` | `MenuItem`, `MenuFile` 追加、`MenuContent` 変更、`resolve_menu_files` 修正、`DialogContent::CustomFunctionMenu` 追加 |
| `rwf-bin/src/ui/dialog/mod.rs` | カスタム関数ダイアログ Enter 処理変更、メニューダイアログ描画・キー処理追加 |
| `rwf-lib/src/input/mod.rs` or `state.rs` | `ExecuteMenuItem` 遷移追加、Action 名解決ロジック |
| `rwf-lib/src/config.rs` or `state.rs` | menu ファイル起動時バリデーション |
| `sample/custom_functions.json` | `Menu` フィールドのインライン形式を `MenuItem` スタイルに修正 |

---

## 検証手順

1. `custom_functions.json` に `"Menu": "menu_file_operations.json"` を持つ関数を定義
2. `T`（Shift+T）でカスタム関数ダイアログを開く
3. メニュー型関数を選択して Enter → メニューダイアログが開く
4. Up/Down でナビゲーション（セパレータがスキップされること）
5. 文字キーでジャンプ（例: `O` → "Open in Notepad" へ）
6. Enter でカスタム関数実行 or ビルトインアクション実行
7. Esc でダイアログ全閉じ（カスタム関数リストには戻らない）
8. F2（verbose version info）で menu ファイルが `[OK]`/`[NG]` と表示されること

---

## 未対応事項（将来フェーズ）

- menu ファイルから別の menu ファイルを開く（TWF も未対応）
- カスタム関数ダイアログへの「戻る」ナビゲーション（TWF の仕様にない）
- メニューダイアログ内でのインクリメンタル検索
