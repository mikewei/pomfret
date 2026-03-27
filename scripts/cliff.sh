#!/bin/sh

set -e

git cliff v0.2.0..HEAD

echo """
## [0.2.0] - 2026-03-20

- Support Google Gemini backend
- Fix some bugs


## [0.1.1] - 2026-3-13

- Improve UI and UX
- Fix some bugs
- Add some unit tests

"""