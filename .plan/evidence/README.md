# Evidence records

One file per task: `QM-XXXX.md`.

Controller §1 substitutes these for the pull-request bodies `.plan/README.md`
assumes. No pull request is creatable in this repository (the `gh` token has
`push: false`), so the evidence record carries what the PR body would have
carried — diff summary, acceptance-criteria mapping, tests added with real counts,
negative paths, the independent reviewer's verdict and the SHA it reviewed, and the
explicit limits of what the change establishes.

The evidence record, the `STATUS.md` rows, and the implementation land in the same
squash commit. That commit is the review artifact.
