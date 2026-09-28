/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

import { themes } from 'prism-react-renderer';
import type { ThemeConfig as ClassicPresetConfig, Options as ClassicPresetOptions } from '@docusaurus/preset-classic';
import type { DocusaurusConfig } from '@docusaurus/types';

import { postProcessItems } from './sidebars.js';
import { redirects } from './redirects';

const lightCodeTheme = themes.github;
const darkCodeTheme = themes.dracula;

const presetOptions: ClassicPresetOptions = ({
  docs: {
    path: 'docs',
    sidebarPath: require.resolve('./sidebars_generated.ts'),
    async sidebarItemsGenerator({ defaultSidebarItemsGenerator, ...args }) {
      const items = await defaultSidebarItemsGenerator({
        ...args
      });
      return postProcessItems(items);
    },
  },
  theme: {
    customCss: require.resolve('./src/css/custom.css'),
  },
});

const themeConfig: ClassicPresetConfig = ({
  docs: {
    sidebar: {
      hideable: true,
    },
  },
  navbar: {
    title: 'Buck2',
    logo: {
      alt: 'Buck2 Logo',
      src: 'img/logo.svg',
    },
    items: [
      {
        type: 'doc',
        docId: 'index',
        position: 'left',
        label: 'Docs',
      },
      {
        to: '/docs/api',
        position: 'left',
        label: 'API',
        activeBaseRegex: '/docs/api',
      },
      {
        to: '/docs/prelude/rules',
        position: 'left',
        label: 'Rules',
        activeBasePath: '/docs/prelude',
      },
      {
        href: 'https://github.com/rdeusser/buck2',
        label: 'GitHub',
        position: 'right',
      },
    ],
  },
  footer: {
    style: 'dark',
    links: [
      {
        title: 'Docs',
        items: [
          {
            label: 'User guide',
            to: '/docs',
          },
        ],
      },
      {
        title: 'Community',
        items: [
          {
            label: 'GitHub issues',
            href: 'https://github.com/rdeusser/buck2/issues',
          },
        ],
      },
      {
        title: 'More',
        items: [
          {
            label: 'Code',
            href: 'https://github.com/rdeusser/buck2',
          },
        ],
      },
    ],
    copyright: 'Based on Buck2, © Meta Platforms, Inc. and affiliates. Built with Docusaurus.',
  },
  prism: {
    additionalLanguages: ['bash', 'powershell', 'cpp', 'ini', 'mermaid'],
    theme: lightCodeTheme,
    darkTheme: darkCodeTheme,
  },
});

const config: DocusaurusConfig = ({
  title: 'Buck2',
  // GitHub Pages serves a project site from https://<owner>.github.io/<repository>/.
  url: 'https://rdeusser.github.io',
  baseUrl: '/buck2/',
  onBrokenLinks: 'throw',
  trailingSlash: true,
  onBrokenMarkdownLinks: 'warn',
  favicon: 'img/logo.png',
  organizationName: 'rdeusser',
  projectName: 'buck2',

  presets: [
    [
      '@docusaurus/preset-classic',
      presetOptions,
    ],
  ],

  plugins: [
    [
      '@docusaurus/plugin-client-redirects',
      {
        redirects: redirects,
      },
    ],
  ],

  themeConfig,

  // @ts-ignore : Fields of this are not declared as optional, but they are
  markdown: ({
    // Use mdx for `.mdx` files and commonmark for `.md` files
    format: 'mdx',
    mermaid: true,
  }),
  themes: ['@docusaurus/theme-mermaid'],
});

module.exports = {
  config: config,
};
