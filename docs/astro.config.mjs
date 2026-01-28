import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	base: '/blazebee/',
	redirects: {
		'/': 'getting-started/linux',
	},
	integrations: [
		starlight({
			title: 'BlazeBee Documentation',
			description: 'Lightning-fast system metrics collector for modern infrastructure',
			social: [
				{
					label: 'GitHub',
					href: 'https://github.com/rubtsov-stan/blazebee',
					icon: 'github',
				},
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Installation', slug: 'getting-started/linux' },
						{ label: 'Docker Deployment', slug: 'getting-started/docker-compose' },
						{ label: 'Kubernetes', slug: 'getting-started/kubernetes' },
						{ label: 'Security', slug: 'getting-started/security' },

					],
				},
				{
					label: 'Configuration',
					items: [
						{ label: 'Configuration Guide', slug: 'configuration/overview' },
						{ label: 'MQTT Settings', slug: 'configuration/mqtt' },
					],
				},
				{
					label: 'Metrics Reference',
					items: [
						{ label: 'System Metrics', slug: 'metrics/system' },
						{ label: 'Advanced Metrics', slug: 'metrics/advanced' },
						{ label: 'Custom Collectors', slug: 'metrics/custom' },
					],
				},
				{
					label: 'Development',
					items: [
						{ label: 'Building from Source', slug: 'development/build' },
						{ label: 'Architecture', slug: 'development/architecture' },
						{ label: 'Contributing', slug: 'development/contributing' },
					],
				},
			],
			head: [
				{
					tag: 'meta',
					attrs: {
						name: 'theme-color',
						content: '#ff6b00',
					},
				},
			],
		}),
	],
	customCss: ['./src/styles/custom.css'],

});