export type BannerGraphic = {
  id: string
  src: string
  title: string
  description: string
  artist: string
  year: string
  objectPosition: `${number}% ${number}%`
}

export const BANNER_GRAPHICS = [
  {
    id: 'graphic-01',
    src: '/banners/graphic-01.webp',
    title: 'Incoming Tide, Scarboro, Maine',
    description: "Waves crash against rugged shores of Prout's Neck, Maine.",
    artist: 'Winslow Homer',
    year: '1883',
    objectPosition: '50% 70%',
  },
  {
    id: 'graphic-02',
    src: '/banners/graphic-02.webp',
    title: 'Boy with Anchor',
    description:
      "A lone boy hauls an anchor on the shore, evoking the weight of maritime destiny and Homer's mastery of atmosphere and technique.",
    artist: 'Winslow Homer',
    year: '1873',
    objectPosition: '50% 35%',
  },
  {
    id: 'graphic-03',
    src: '/banners/graphic-03.webp',
    title: 'Seagull and Waves',
    description: 'Study of rippling waves in a body of water.',
    artist: 'Winslow Homer',
    year: 'ca. 1884',
    objectPosition: '50% 50%',
  },
  {
    id: 'graphic-04',
    src: '/banners/graphic-04.webp',
    title: 'A Good Pool, Saguenay River',
    description:
      'Three fishermen in a canoe reel out a disproportionately large fish from the sea.',
    artist: 'Winslow Homer',
    year: '1896',
    objectPosition: '50% 80%',
  },
  {
    id: 'graphic-05',
    src: '/banners/graphic-05.webp',
    title: 'Hudson River, Logging',
    description:
      'A celebrated watercolor painting depicting two men maneuvering logs on a vibrant blue river amidst Adirondack mountains.',
    artist: 'Winslow Homer',
    year: '1891-1892',
    objectPosition: '50% 60%',
  },
  {
    id: 'graphic-06',
    src: '/banners/graphic-06.webp',
    title: 'Summer Squall',
    description:
      'A dramatic oil-on-canvas seascape depicting the intense, violent power of a sudden storm crashing against the Maine coast.',
    artist: 'Winslow Homer',
    year: '1904',
    objectPosition: '50% 10%',
  },
  {
    id: 'graphic-07',
    src: '/banners/graphic-07.webp',
    title: 'Maine Coast',
    description:
      "A rugged Maine coastscape. Expressive brushwork and realism evoke the ocean's power and grandeur.",
    artist: 'Winslow Homer',
    year: '1896',
    objectPosition: '50% 25%',
  },
  {
    id: 'graphic-08',
    src: '/banners/graphic-08.webp',
    title: 'Boys in a Dory',
    description: 'Three boys rest in a small rowboat against a calm, bright sea.',
    artist: 'Winslow Homer',
    year: '1880',
    objectPosition: '50% 50%',
  },
  {
    id: 'graphic-09',
    src: '/banners/graphic-09.webp',
    title: 'Sloop, Nassau',
    description:
      'A single-masted sailboat glides through the vibrant, turquoise waters of the Bahamas.',
    artist: 'Winslow Homer',
    year: '1899',
    objectPosition: '50% 48%',
  },
  {
    id: 'graphic-10',
    src: '/banners/graphic-10.webp',
    title: 'Northeaster',
    description:
      'Foamy waves crash against dark, rugged rocks against the coast of a wintery Maine.',
    artist: 'Winslow Homer',
    year: '1895',
    objectPosition: '50% 50%',
  },
  {
    id: 'graphic-11',
    src: '/banners/graphic-11.webp',
    title: 'The Wrecked Schooner',
    description: 'A wrecked boat with a collapsed mast lies abandoned against an icy coast.',
    artist: 'Winslow Homer',
    year: 'ca. 1900-1910',
    objectPosition: '50% 60%',
  },
  {
    id: 'graphic-12',
    src: '/banners/graphic-12.webp',
    title: 'Fishing Boats, Key West',
    description: 'Bahamian fishing boats drift on an aquamarine sea under sun and breeze.',
    artist: 'Winslow Homer',
    year: '1903',
    objectPosition: '50% 40%',
  },
  {
    id: 'graphic-13',
    src: '/banners/graphic-13.webp',
    title: 'Key West, Hauling Anchor',
    description:
      'A white boat with lowered sails floats in blue-green water beneath a pale blue sky, with a few people in the bow and distant palm trees on the horizon.',
    artist: 'Winslow Homer',
    year: '1903',
    objectPosition: '50% 65%',
  },
  {
    id: 'graphic-14',
    src: '/banners/graphic-14.webp',
    title: 'The End of the Day, Adirondacks',
    description:
      'A fisherman sits in a canoe at sunset on a lake with pink and orange sky reflected in the misty water.',
    artist: 'Winslow Homer',
    year: '1890',
    objectPosition: '50% 60%',
  },
] satisfies [BannerGraphic, ...BannerGraphic[]]

export const DEFAULT_BANNER_GRAPHIC = BANNER_GRAPHICS[0]
export const DEFAULT_ROTATING_BANNER_GRAPHIC_ID = 'graphic-02'

export function getBannerGraphic(src: string): BannerGraphic {
  return (
    BANNER_GRAPHICS.find((graphic) => graphic.src === src) ?? {
      id: src,
      src,
      title: 'Banner artwork',
      description: 'Decorative banner artwork.',
      artist: 'Winslow Homer',
      year: 'Unknown',
      objectPosition: '50% 50%',
    }
  )
}
