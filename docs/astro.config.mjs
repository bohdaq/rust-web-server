import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sidebar from './sidebar.json' with { type: 'json' };

export default defineConfig({
  integrations: [
    starlight({
      title: 'rust-web-server',
      description:
        'HTTP/1.1 · HTTP/2 · HTTP/3/QUIC · Reverse Proxy · MCP Server · ORM — no hyper, no Tower, one crate.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
      },
      logo: {
        src: './src/assets/logo.svg',
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/bohdaq/rust-web-server' },
      ],
      customCss: ['./src/styles/custom.css'],
      // docs/sidebar.json is the one sidebar: ci/gen_llms.py and site/ read it too.
      sidebar,
    }),
  ],
});
