import { defineConfig } from 'vitepress'

const enSidebar = [
  { text: 'Overview', link: '/' },
  { text: 'Architecture', link: '/architecture' },
  { text: 'Implementation', link: '/implementation' },
  { text: 'Visualizer', link: '/visualizer' },
  { text: 'Capture Store', link: '/capture-store' }
]

const zhSidebar = [
  { text: '概览', link: '/zh/' },
  { text: '项目架构', link: '/zh/architecture' },
  { text: '实现细节', link: '/zh/implementation' },
  { text: 'Visualizer 使用说明', link: '/zh/visualizer' },
  { text: 'Capture Store 使用说明', link: '/zh/capture-store' }
]

export default defineConfig({
  title: 'Quanergy Client RS',
  description: 'Rust rewrite documentation for the Quanergy client SDK data path.',
  cleanUrls: true,
  themeConfig: {
    search: {
      provider: 'local'
    }
  },
  locales: {
    root: {
      label: 'English',
      lang: 'en-US',
      title: 'Quanergy Client RS',
      description: 'Rust rewrite documentation for the Quanergy client SDK data path.',
      themeConfig: {
        nav: [
          { text: 'Architecture', link: '/architecture' },
          { text: 'Implementation', link: '/implementation' },
          { text: 'Visualizer', link: '/visualizer' },
          { text: 'Capture Store', link: '/capture-store' }
        ],
        sidebar: enSidebar
      }
    },
    zh: {
      label: '简体中文',
      lang: 'zh-CN',
      link: '/zh/',
      title: 'Quanergy Client RS',
      description: 'Quanergy 客户端 SDK 数据链路的 Rust 重写文档。',
      themeConfig: {
        nav: [
          { text: '项目架构', link: '/zh/architecture' },
          { text: '实现细节', link: '/zh/implementation' },
          { text: 'Visualizer', link: '/zh/visualizer' },
          { text: 'Capture Store', link: '/zh/capture-store' }
        ],
        sidebar: zhSidebar,
        outline: {
          label: '页面导航'
        },
        docFooter: {
          prev: '上一页',
          next: '下一页'
        },
        returnToTopLabel: '回到顶部',
        sidebarMenuLabel: '菜单',
        darkModeSwitchLabel: '深色模式',
        lightModeSwitchTitle: '切换到浅色模式',
        darkModeSwitchTitle: '切换到深色模式'
      }
    }
  }
})
