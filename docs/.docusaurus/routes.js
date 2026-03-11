import React from 'react';
import ComponentCreator from '@docusaurus/ComponentCreator';

export default [
  {
    path: '/docs',
    component: ComponentCreator('/docs', 'a06'),
    routes: [
      {
        path: '/docs',
        component: ComponentCreator('/docs', 'e55'),
        routes: [
          {
            path: '/docs',
            component: ComponentCreator('/docs', '81e'),
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
                path: '/docs/renderer',
                component: ComponentCreator('/docs/renderer', '735'),
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
