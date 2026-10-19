import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'zh-CN',
  title: 'onTime',
  description: 'onTime · 轻量桌面提醒工具。要紧的事，到点提醒。',
  head: [['link', { rel: 'icon', type: 'image/svg+xml', href: '/onTime/brand/app-icon.svg' }]],
  base: '/onTime/',
  themeConfig: {
    logo: '/brand/app-icon.svg',
    nav: [{ text: '功能', link: '/features' }, { text: '开始使用', link: '/guide' }, { text: '更新日志', link: '/changelog' }, { text: '常见问题', link: '/faq' }, { text: '下载', link: 'https://github.com/yeyongzhi/onTime/releases' }],
    socialLinks: [{ icon: 'github', link: 'https://github.com/yeyongzhi/onTime' }],
    search: { provider: 'local' },
    outline: { label: '本页内容' },
    docFooter: { prev: '上一页', next: '下一页' },
    footer: { message: '把提醒留在桌面，把注意力留给生活。', copyright: 'onTime' },
  },
})
