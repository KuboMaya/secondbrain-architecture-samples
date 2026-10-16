# tests-samples

本番リポジトリの `src-tauri/tests/` と同じ方針で書いた **読み物／静的解析向け** テスト群です。

| 本番 | 公開サンプル |
|------|----------------|
| `tests/common/mock_db.rs` | `common/test_harness.rs`（shared-cache memory + 最小スキーマ） |
| `tests/reward/math_tests.rs` + `mcdc_tests.rs` | `domains/reward/gacha_math.rs` 内 `#[cfg(test)]` |
| `tests/reward/event_tests.rs` | `reward_bus_integration.rs`（EventBus→gacha の非同期待ち） |
| `tests/sync/conflict_tests.rs` | `lww_conflict_tests.rs`（境界・409 相当の unit） |
| （FE） | `frontend-sync-samples/sync/lwwApply.test.ts` |

意図:
- 単なる `assert!(true)` ではなく、**分岐・境界・非同期待ち合わせ**を本番に近い形で見せる
- 公開リポ単体では依存クレートが無いため **そのまま `cargo test` は想定外**（設計・複雑度の証拠として読ませる）
