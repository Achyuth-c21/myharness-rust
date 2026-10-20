# Live-Patch Evidence Summary

**App version:** `2.42304.0`  
**Method:** recursive byte comparison of the pristine extracted `app.asar.orig` tree and the live
extracted `app.asar` tree, plus inspection of the external inference bridge.

Only two files differ inside the asar:

| File | Pristine SHA-256 | Live SHA-256 |
|---|---|---|
| `.vite/build/index.pre.js` | `44be1fef74fab957ad5e8e5e3589ee1b7d5e94043e473de9b0e8f3d551de5f06` | `0263f3e12504e16a3b166a04342d170ba71b545af18295a546316cd86626eb52` |
| `.vite/build/index.chunk-Z7WxBjyE.js` | `e17420a6e61c7cacdcc39ad77849291bfb6f423a40dad2ab1a95c1d91848cae1` | `22d81c0701dc9e9dc0a4d73c92216833d3dd98f023fbf25b02bf75c02eaeba2e` |

Observed changes, with deployment-specific paths redacted:

1. `index.pre.js` inserts a bootstrap `require(<user-config>/inference-bridge/bridge.cjs)` guarded by
   `try/catch`; failure logs `[inference-bridge] bootstrap failed`.
2. `index.chunk-Z7WxBjyE.js` wraps a fetch path with
   `globalThis.__claudeNative?.shouldTranslate?.(...)` and delegates to
   `globalThis.__claudeNative.fetch(...)` when true.
3. The same chunk replaces the shipped model-policy predicate with `function So(e){return!0}`,
   disabling the Claude/Anthropic model-name restriction.

The bridge loads an external adapter outside the asar:

| Artifact | Lines | SHA-256 | Status |
|---|---:|---|---|
| `native-adapter.cjs` | 1,001 | `b1a30f68c23380b74b8c80f34ffaf1b7e559e33f7fc47925c7d173430b56e76b` | **Noncompliant** Anthropic→OpenAI translation layer |
| `bridge.cjs` | — | `caea1816cc5d9649e91424178a36acdf46ded99250f2dd295fb4087053ce8046` | Local patch bootstrap |

The adapter and patched source trees are not part of the publishable repository. This summary and the
documented behavior are the approved evidence. The adapter drops thinking/cache metadata, drops
tool-result images, and bypasses provider/model validation; the rebuild must not reproduce it.
