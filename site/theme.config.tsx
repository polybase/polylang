import React from 'react'
import { DocsThemeConfig } from 'nextra-theme-docs'
import { Logo } from './components/Logo'

const config: DocsThemeConfig = {
  logo: <Logo fontSize='xl' size={40} />,
  primaryHue: 314,
  project: {
    link: 'https://github.com/polybase/polylang/',
  },
  chat: {
    link: 'https://discord.com/invite/DrXkRpCFDX',
  },
  docsRepositoryBase: 'https://github.com/polybase/polylang/tree/main/site/',
  banner: {
    key: '2.0-release',
    text: (
      <a href="https://polybase.notion.site/Halo2-Engineer-Rust-ccea568bbfd64afa81d548d5c6fc429e" target="_blank">
        🎉 We're hiring ZK (Halo2) developers. Read more →
      </a>
    )
  },
  useNextSeoProps() {
    return {
      titleTemplate: '%s – Polylang by Polybase Labs'
    }
  },
  head: (
    <>
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <meta property="og:title" content="Polylang" />
      <meta property="og:description" content="TypeScript for Zero Knowledge. The language you know and love ported to run in ZK." />
      <meta property="og:image" content="https://polylang.dev/social.png" />
      <meta name="twitter:site" content="@polybase_xyz"></meta>
      <meta property="twitter:image" content="https://polylang.dev/social.png" />
    </>
  ),
  footer: {
    text: 'Polylang Docs',
  },
}

export default config
