import type { useLifecycle } from './lifecycle';

type Lifecycle = ReturnType<typeof useLifecycle>;

/**
 * The error / success / busy trio every settings page declared for itself,
 * together with the try / catch / finally that always wrapped an IPC call.
 *
 * Roughly thirty handlers repeated the same eight lines, which is how the
 * banners came to be cleared at slightly different moments and how one page
 * ended up scheduling its timers without a key.
 */
export function createStatus(lifecycle: Lifecycle) {
  let error = $state('');
  let success = $state('');
  let busy = $state(false);

  return {
    get error() {
      return error;
    },
    get success() {
      return success;
    },
    /** True while `run` is awaiting an action. */
    get busy() {
      return busy;
    },

    /** Clear both banners, e.g. when a page switches what it is showing. */
    reset() {
      error = '';
      success = '';
    },

    /**
     * Report a failure the caller detected itself, such as a validation rule.
     * Pass `clearAfterMs` for a message that should not linger.
     */
    fail(message: string, clearAfterMs?: number) {
      error = message;
      if (clearAfterMs !== undefined) {
        lifecycle.timeout(() => {
          error = '';
        }, clearAfterMs, 'error');
      }
    },

    /** Show a confirmation that clears itself. */
    confirm(message: string, clearAfterMs = 3000) {
      success = message;
      lifecycle.timeout(() => {
        success = '';
      }, clearAfterMs, 'success');
    },

    /**
     * Run one action: clear the banners, mark the page busy, and turn a
     * rejection into `${describe}: ${cause}` on the error banner.
     *
     * Resolves to `undefined` when the action failed, so a caller that needs
     * to branch can check `status.error` rather than nesting another try.
     */
    async run<T>(describe: string, job: () => Promise<T>): Promise<T | undefined> {
      error = '';
      success = '';
      busy = true;
      try {
        return await job();
      } catch (cause) {
        error = `${describe}: ${cause}`;
        console.error(error);
        return undefined;
      } finally {
        busy = false;
      }
    },
  };
}
