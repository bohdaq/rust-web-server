import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: 'rust-web-server',
      description:
        'HTTP/1.1 · HTTP/2 · HTTP/3/QUIC · Reverse Proxy · MCP Server · ORM — no third-party HTTP dependencies.',
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
      },
      logo: {
        src: './src/assets/logo.svg',
      },
      social: {
        github: 'https://github.com/bohdaq/rust-web-server',
      },
      customCss: ['./src/styles/custom.css'],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Quick Start',   link: '/getting-started/quick-start/' },
            { label: 'Installation',  link: '/getting-started/installation/' },
            { label: 'All Features',  link: '/getting-started/features/' },
          ],
        },
        {
          label: 'Configuration',
          items: [
            { label: 'Overview',             link: '/configuration/overview/' },
            { label: 'Environment Variables', link: '/configuration/env-vars/' },
            { label: 'Config File',           link: '/configuration/config-file/' },
            { label: 'CLI Args',              link: '/configuration/cli-args/' },
          ],
        },
        {
          label: 'Building Apps',
          collapsed: true,
          items: [
            { label: 'Overview',              link: '/building-apps/overview/' },
            { label: 'Controllers',           link: '/building-apps/controllers/' },
            { label: 'Routing',               link: '/building-apps/routing/' },
            { label: 'Request & Response',    link: '/building-apps/request-response/' },
            { label: 'Shared State',          link: '/building-apps/state/' },
            { label: 'Typed Extractors',      link: '/building-apps/extractors/' },
            { label: 'Error Handling',        link: '/building-apps/error-handling/' },
            { label: 'Middleware',            link: '/building-apps/middleware/' },
            { label: 'Validation',            link: '/building-apps/validation/' },
            { label: 'Forms & Uploads',       link: '/building-apps/forms-uploads/' },
            { label: 'JSON',                  link: '/building-apps/json/' },
            { label: 'Cookies',               link: '/building-apps/cookies/' },
            { label: 'Async Handlers',        link: '/building-apps/async-handlers/' },
            { label: 'HTML Templates',        link: '/building-apps/templates/' },
            { label: 'Dependency Injection',  link: '/building-apps/dependency-injection/' },
          ],
        },
        {
          label: 'Features',
          collapsed: true,
          items: [
            { label: 'CORS & Security',        link: '/features/cors-security/' },
            { label: 'Rate Limiting',          link: '/features/rate-limiting/' },
            { label: 'Compression',            link: '/features/compression/' },
            { label: 'HTTPS / TLS',            link: '/features/https-tls/' },
            { label: 'Automatic TLS (ACME)',   link: '/features/acme/' },
            { label: 'Mutual TLS (mTLS)',      link: '/features/mtls/' },
            { label: 'Virtual Hosting',        link: '/features/virtual-hosting/' },
            { label: 'HTTP/2',                 link: '/features/http2/' },
            { label: 'HTTP/3 / QUIC',          link: '/features/http3-quic/' },
            { label: 'WebSocket',              link: '/features/websocket/' },
            { label: 'Server-Sent Events',     link: '/features/sse/' },
            { label: 'Auth',                   link: '/features/auth/' },
            { label: 'OAuth2 / OIDC SSO',      link: '/features/sso/' },
            { label: 'CSRF Protection',        link: '/features/csrf/' },
            { label: 'Sessions',               link: '/features/sessions/' },
            { label: 'Response Caching',       link: '/features/caching/' },
            { label: 'Metrics',                link: '/features/metrics/' },
            { label: 'Distributed Tracing',    link: '/features/tracing/' },
            { label: 'Hot Config Reload',      link: '/features/hot-reload/' },
            { label: 'Rewrite Middleware',     link: '/features/rewrite/' },
            { label: 'IP Filtering',           link: '/features/ip-filter/' },
            { label: 'Background Scheduler',   link: '/features/scheduler/' },
            { label: 'Typed Config Binding',   link: '/features/config-binding/' },
          ],
        },
        {
          label: 'Proxy / Gateway',
          collapsed: true,
          items: [
            { label: 'Overview',               link: '/proxy/overview/' },
            { label: 'Config-Driven Proxy',    link: '/proxy/config-driven/' },
            { label: 'Reverse Proxy',          link: '/proxy/reverse-proxy/' },
            { label: 'Load Balancing',         link: '/proxy/load-balancing/' },
            { label: 'Health Checks',          link: '/proxy/health-checks/' },
            { label: 'Circuit Breaker',        link: '/proxy/circuit-breaker/' },
            { label: 'Canary / Traffic Split', link: '/proxy/canary/' },
            { label: 'Service Discovery',      link: '/proxy/service-discovery/' },
            { label: 'TCP Proxy',              link: '/proxy/tcp-proxy/' },
            { label: 'UDP Proxy',              link: '/proxy/udp-proxy/' },
            { label: 'WebSocket Proxy',        link: '/proxy/websocket-proxy/' },
            { label: 'gRPC Proxy',             link: '/proxy/grpc-proxy/' },
          ],
        },
        {
          label: 'Database / ORM',
          collapsed: true,
          items: [
            { label: 'Overview',             link: '/database/overview/' },
            { label: '#[derive(Model)]',     link: '/database/model-derive/' },
            { label: 'Repository',           link: '/database/repository/' },
            { label: 'Query Builder',        link: '/database/query-builder/' },
            { label: 'Raw SQL',              link: '/database/raw-sql/' },
            { label: 'Transactions',         link: '/database/transactions/' },
            { label: 'Migrations',           link: '/database/migrations/' },
            { label: 'Relations',            link: '/database/relations/' },
          ],
        },
        {
          label: 'MCP Server',
          collapsed: true,
          items: [
            { label: 'Overview',   link: '/mcp/overview/' },
            { label: 'Tools',      link: '/mcp/tools/' },
            { label: 'Resources',  link: '/mcp/resources/' },
            { label: 'Prompts',    link: '/mcp/prompts/' },
            { label: 'Auth',       link: '/mcp/auth/' },
          ],
        },
        {
          label: 'Testing',
          items: [
            { label: 'Test Client', link: '/testing/test-client/' },
          ],
        },
        {
          label: 'Deployment',
          collapsed: true,
          items: [
            { label: 'Docker',               link: '/deployment/docker/' },
            { label: 'Kubernetes',           link: '/deployment/kubernetes/' },
            { label: 'Kubernetes Ingress',   link: '/deployment/kubernetes-ingress/' },
            { label: 'Observability',        link: '/deployment/observability/' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'API Reference', link: '/reference/api/' },
            { label: 'Roadmap',       link: '/reference/roadmap/' },
          ],
        },
      ],
    }),
  ],
});
