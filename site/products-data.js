/**
 * Product catalog for tulz.org/products
 *
 * To add a new product:
 *   1. Add an object to the PRODUCTS array below
 *   2. That's it — the page renders dynamically from this data
 */

const PRODUCTS = [
  {
    id: 'sf-toolkit',
    name: 'Advanced Salesforce Developer Toolkit',
    shortName: 'SF Developer Toolkit',
    tagline: 'Salesforce developer & admin toolkit: record inspector, SOQL editor, metadata search, debug logs, data builder.',
    category: 'extension',
    version: '1.1.0',
    badge: null,
    icon: 'sf-toolkit',
    platforms: ['chrome'],
    features: [
      'Global Search Palette',
      'Record Inspector',
      'SOQL Query Tool',
      'Debug Log Analyzer',
      'Execute Anonymous Apex',
      'Smart Navigator',
    ],
    links: {
      chrome: 'https://chromewebstore.google.com/detail/advanced-salesforce-devel/dmaijjgogckglbleglkplaihlmjbhgif',
    },
  },
  {
    id: 'conga-debugger',
    name: 'Conga Debugger',
    shortName: 'Conga Debugger',
    tagline: 'DevTools extension for debugging Conga CPQ network traffic, WebSocket messages, and API requests in real time.',
    category: 'extension',
    version: '1.0',
    badge: null,
    icon: 'conga-debugger',
    platforms: ['chrome'],
    features: [
      'Real-time Network Monitoring',
      'Smart Rules Engine',
      'Field History Tracking',
      'JSON Comparison',
      'Log File Analyzer',
      'Request Re-trigger',
    ],
    links: {
      chrome: 'https://chromewebstore.google.com/detail/conga-debugger/ibppeianghcdobpeblbpmkdlefbplnih',
    },
  },
  {
    id: 'hush',
    name: 'Hush',
    shortName: 'Hush',
    tagline: 'One-click Do Not Disturb for your desktop. Sits in your menu bar and auto-silences notifications when you join a meeting.',
    category: 'app',
    version: '1.0.0',
    badge: 'new',
    icon: 'hush',
    platforms: ['macos', 'windows'],
    features: [
      'One-click DND Toggle',
      'Auto-DND on Calls',
      'Menu Bar App',
      'Free & Open Source',
    ],
    links: {
      download: {
        macos: 'https://github.com/srikanthpullela/hush-app/releases/latest/download/Hush_1.0.0_aarch64.dmg',
        windows: 'https://github.com/srikanthpullela/hush-app/releases/latest/download/Hush_1.0.0_x64-setup.exe',
      },
    },
  },
];
