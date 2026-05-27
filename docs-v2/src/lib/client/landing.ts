import { mountHorizontalScrollFades } from './horizontal-scroll-fade'
import { mountLandingScrollCue } from './landing-scroll-cue'

export function mountLandingPage(): void {
  mountHorizontalScrollFades()
  mountLandingScrollCue()
}
