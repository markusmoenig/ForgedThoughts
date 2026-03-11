import React from 'react';
import ComponentCreator from '@docusaurus/ComponentCreator';

export default [
  {
    path: '/__docusaurus/debug',
    component: ComponentCreator('/__docusaurus/debug', '5ff'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/config',
    component: ComponentCreator('/__docusaurus/debug/config', '5ba'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/content',
    component: ComponentCreator('/__docusaurus/debug/content', 'a2b'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/globalData',
    component: ComponentCreator('/__docusaurus/debug/globalData', 'c3c'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/metadata',
    component: ComponentCreator('/__docusaurus/debug/metadata', '156'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/registry',
    component: ComponentCreator('/__docusaurus/debug/registry', '88c'),
    exact: true
  },
  {
    path: '/__docusaurus/debug/routes',
    component: ComponentCreator('/__docusaurus/debug/routes', '000'),
    exact: true
  },
  {
    path: '/docs',
    component: ComponentCreator('/docs', '1c6'),
    routes: [
      {
        path: '/docs',
        component: ComponentCreator('/docs', 'f7a'),
        routes: [
          {
            path: '/docs',
            component: ComponentCreator('/docs', 'ba7'),
            routes: [
              {
                path: '/docs/',
                component: ComponentCreator('/docs/', 'b43'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/booleans',
                component: ComponentCreator('/docs/booleans', '327'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/cli',
                component: ComponentCreator('/docs/cli', 'e2e'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/install',
                component: ComponentCreator('/docs/install', '348'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/language',
                component: ComponentCreator('/docs/language', 'fef'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/lights',
                component: ComponentCreator('/docs/lights', '5d7'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/limitations',
                component: ComponentCreator('/docs/limitations', 'd9a'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/materials',
                component: ComponentCreator('/docs/materials', '730'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/objects',
                component: ComponentCreator('/docs/objects', '5b3'),
                exact: true,
                sidebar: "mainSidebar"
              },
              {
                path: '/docs/settings',
                component: ComponentCreator('/docs/settings', 'e94'),
                exact: true,
                sidebar: "mainSidebar"
              }
            ]
          }
        ]
      }
    ]
  },
  {
    path: '/',
    component: ComponentCreator('/', 'e5f'),
    exact: true
  },
  {
    path: '*',
    component: ComponentCreator('*'),
  },
];
