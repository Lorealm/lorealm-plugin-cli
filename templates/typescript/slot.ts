/// <reference types="@lorealm/plugin/frame" />
import type { LorealmApi, {{context}} } from '@lorealm/plugin';

// `lorealm` is the client's API, defined globally in a frame. This narrows its
// context to this slot's.
const api = lorealm as LorealmApi<{{context}}>;

const hello = document.getElementById('hello')!;
const answer = document.getElementById('answer')!;

// `call` reaches this plugin's own backend, in `frontend`.
hello.addEventListener('click', async () => {
  try {
    answer.textContent = String(await api.call('hello'));
  } catch (e) {
    answer.textContent = e instanceof Error ? e.message : String(e);
  }
});
