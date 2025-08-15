export default defineNuxtConfig({
  compatibilityDate: '2024-12-20',
  
  // Enable the Nuxt devtools
  devtools: { enabled: true },
  
  // Enable SSG for Tauri
  ssr: false,
  
  // Modules
  modules: [
    '@nuxt/ui'
  ],
  
  // UI Configuration
  ui: {
    global: true,
    icons: ['heroicons', 'simple-icons'],
    theme: {
      colors: ['green', 'blue', 'purple', 'pink', 'orange']
    }
  },
  
  // Tailwind CSS configuration
  tailwindcss: {
    config: {
      theme: {
        extend: {
          fontFamily: {
             sans: [
               'Noto Sans SC',
               'PingFang SC',
               'Microsoft YaHei',
               'Hiragino Sans GB',
               'Source Han Sans CN',
               'Noto Sans CJK SC',
               'WenQuanYi Micro Hei',
               'Apple Color Emoji',
               'Segoe UI Emoji',
               'Segoe UI Symbol',
               'Noto Color Emoji',
               'sans-serif'
             ],
            mono: [
              'JetBrains Mono',
              'Fira Code',
              'SF Mono',
              'Monaco',
              'Inconsolata',
              'Roboto Mono',
              'Source Code Pro',
              'Menlo',
              'Consolas',
              'DejaVu Sans Mono',
              'monospace'
            ],
            numeric: [
               'Noto Sans SC',
               'SF Pro Display',
               'system-ui',
               '-apple-system',
               'BlinkMacSystemFont',
               'Segoe UI',
               'Roboto',
               'sans-serif'
             ]
          }
        }
      }
    }
  },
  
  // CSS
  css: ['~/assets/css/main.css'],
  
  // Enables the development server to be discoverable by other devices when running on iOS physical devices
  devServer: {
    host: '127.0.0.1',
    port: 1420
  },
  
  vite: {
    // Better support for Tauri CLI output
    clearScreen: false,
    // Enable environment variables
    // Additional environment variables can be found at
    // https://v2.tauri.app/reference/environment-variables/
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      // Tauri requires a consistent port
      strictPort: true,
      hmr: {
        port: 1421
      }
    }
  },
  
  // Avoids error [unhandledRejection] EMFILE: too many open files, watch
  ignore: ['**/src-tauri/**'],
  
  // App configuration
  app: {
    head: {
      title: 'RuRay - Xray Core Desktop Client',
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        { name: 'description', content: 'A modern desktop client for Xray-core proxy software' }
      ]
    }
  }
})