# Skills for Spock users

This directory is the public skill catalog for people using Spock in their own
projects. It is not the agent configuration for contributors working on the
Spock repository.

Install the catalog from GitHub and select `spock-lang` or `uhura-lang` when
prompted:

```sh
npx skills add gridaco/spock
```

To select the skill directly:

```sh
npx skills add gridaco/spock --skill spock-lang
npx skills add gridaco/spock --skill uhura-lang
```

The `spock-lang` skill helps coding agents create, inspect, validate, run,
debug, and modify Spock programs.

The `uhura-lang` skill helps coding agents create, inspect, validate, run,
debug, and modify strict machine-first Uhura 0.4 programs inside
compatible Spock framework projects.

`spock@0.5.4` is the first published package with strict Uhura 0.4 and its
explicit `web-app@1` profile. The skill checks the installed CLI before
authoring source and stops on `0.5.3` or an older incompatible distribution;
it never falls back to the retired v0 language. Building the repository
toolchain is a separate contributor workflow, not part of this public skill.
