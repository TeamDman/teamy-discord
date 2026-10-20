# Parser boundary

The safety wrapper runs before Figue and suppresses its raw argv-bearing errors. Reviewed public unsupported-option names (--mentions, --scan-limit, --token) can be identified, together with a schema-validated command context. Arbitrary unknown labels and all values are omitted. These labels are a fixed public allowlist, never a syntax-based permission to print arbitrary caller strings.

A standalone --help/-h before the -- terminator takes precedence over argument validation, including unknown options and malformed values. Only the deepest validated command prefix selects the help page; traversal freezes at unknown input. --option=--help and tokens after -- remain literal data. Help emits schema usage without executing commands, resolving credentials or opening log files. Removed command names do not regain execution behavior.

Discord is locked to teamy-figue 2.0.1 with Facet 0.44, whereas the reusable template is pinned to figue 5.0.0-rc.5 in the Facet git revision 23ad4dcd034d123f0c38943305ffb077519040ce. The generic unknown-argument message was produced by this wrapper, not evidence of a Figue-version bug. Schema additions and these wrapper fixes do not require a dependency upgrade. A future parser upgrade needs coordinated Facet and consumer compatibility tests.
