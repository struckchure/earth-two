
<!-- harness:runtime v1 -->
## Harness session context

If HARNESS_CONTEXT_FILE is set in your environment, read that file before working:

```sh
if [ -n "$HARNESS_CONTEXT_FILE" ]; then cat "$HARNESS_CONTEXT_FILE"; fi
```

It contains the instructions and skill index for this session's selected harness.
If the variable is absent, this is an ordinary coding session with no selected harness.
<!-- /harness:runtime -->
