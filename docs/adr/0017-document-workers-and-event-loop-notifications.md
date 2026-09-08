# 0017. Document workers notify the native event loop

## Status

Accepted, 2026-09-08.

## Context

The desktop shell could edit only its bundled sample. Opening, saving and
confirming document replacement require disk or modal operations, while the
working agreement forbids disk I/O on the UI thread and recurring polling.
The platform trait exposed only borrowed dialog services and timer wakeups.

## Decision

Extend the platform trait with an owned dialog service and a thread-safe
notification callback. Desktop workers send results through the application's
channel, then notify winit through its existing event-loop proxy. The proxy
continues to carry AccessKit events independently. No worker owns a window
or mutates editor state. The event handler applies completion results.

The current single-document shell serializes file operations. It freezes
editing while replacing or closing a document, but permits edits during a
save. A cheap rope clone captures the save input; encoding and all filesystem
operations execute on the worker. History state IDs identify the version
actually saved, so later edits remain dirty and undo can return to a saved
checkpoint. Save checks the previous disk snapshot before replacing it;
this detects pre-existing external changes but is not a filesystem lock.

Confirmation dialogs carry the parent window. In the windowed macOS app,
RFD dispatches native modal UI to the main thread itself; disk work remains
on the worker. Standalone native dialog tests do not prove this integration.
Current acceptance includes native open, save, cancel-close and save-on-close
in a temporary macOS app bundle, plus headless real-file worker tests.

## Consequences

No timer polls disk or worker completion. A failed or cancelled operation
preserves the current document. Errors and progress appear in the title.
Mobile backends must implement equivalent notifications and owned services
when their real platform implementations land. This does not close multi-tab,
recovery, filesystem watchers, configurable keymaps, native application-quit
events, or cross-platform acceptance.
