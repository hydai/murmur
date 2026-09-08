import './lib/design-tokens.css'
import './app.css'
import './lib/buttons.css'
import { mount } from 'svelte'
import App from './App.svelte'

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
