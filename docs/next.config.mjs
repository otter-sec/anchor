import { createMDX } from 'fumadocs-mdx/next';
import redirectsJson from './redirects.json' with { type: 'json' };

const withMDX = createMDX({
  mdxOptions: {
    lastModifiedTime: 'git',
  },
});

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  webpack: (config) => {
    // Existing config
    config.module.rules.push({
      test: /\.svg$/,
      use: [{ loader: '@svgr/webpack', options: { icon: true } }],
    });

    // Important: return the modified config
    return config;
  },
  async redirects() {
    return [
      // Redirect root to /docs, can remove if a landing page is added
      {
        source: '/',
        destination: '/docs',
        permanent: false,
      },
      // Redirect routes from old docs
      ...redirectsJson.redirects.map((redirect) => ({
        ...redirect,
        permanent: redirect.permanent ?? true,
      })),
    ];
  },
};

export default withMDX(config);
