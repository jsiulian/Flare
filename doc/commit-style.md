# Commit Style

This project loosely follows [conventional commits] with minor changes:

- Always add the scope, e.g. the `messages` in `feat(messages)`.
- We don't use `!` for breaking changes, mostly because depending on your interpretation of "breaking" either most or no changes are breaking in a GUI.

The following types are currently used:

- `build`: Change the way Flare is built.
- `chores`: Maintenance tasks like refactoring.
- `ci`: CI-related changes.
- `doc`: Documentation changes.
- `feat`: New feature.
- `fix`: Bugfix.
- `hotfix`: Very important bugfix.
- `i18n`: Internationalization.
- `perf`: Performance improvements.
- `ui`: UI-related work.
- `ver`: New version.

The following scope can be freely defined based on the code change and there is no predefined set to be used.
