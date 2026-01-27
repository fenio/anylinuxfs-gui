import { writable } from 'svelte/store';
import type { AppConfig } from '../types';
import { getConfig, updateConfig } from '../api';

interface ConfigState {
	config: AppConfig;
	loading: boolean;
	saving: boolean;
	error: string | null;
}

const defaultConfig: AppConfig = {
	ram_mb: null,
	vcpus: null,
	log_level: null
};

function createConfigStore() {
	const { subscribe, set, update } = writable<ConfigState>({
		config: defaultConfig,
		loading: false,
		saving: false,
		error: null
	});

	return {
		subscribe,
		async load() {
			update((s) => ({ ...s, loading: true, error: null }));
			try {
				const config = await getConfig();
				update((s) => ({ ...s, config, loading: false }));
			} catch (e) {
				update((s) => ({ ...s, error: String(e), loading: false }));
			}
		},
		async save(ramMb?: number, vcpus?: number, logLevel?: string) {
			update((s) => ({ ...s, saving: true, error: null }));
			try {
				await updateConfig(ramMb, vcpus, logLevel);
				// Reload config to get the updated values
				const config = await getConfig();
				update((s) => ({ ...s, config, saving: false }));
				return true;
			} catch (e) {
				update((s) => ({ ...s, error: String(e), saving: false }));
				return false;
			}
		},
		clearError() {
			update((s) => ({ ...s, error: null }));
		}
	};
}

export const config = createConfigStore();
