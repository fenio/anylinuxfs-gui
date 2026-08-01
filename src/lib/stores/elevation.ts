import { writable } from 'svelte/store';
import type { ElevationMode, ElevationPolicy } from '../types';
import { getElevationPolicy, setElevationMode } from '../api';
import { parseError } from '../errors';

interface ElevationState {
	policy: ElevationPolicy;
	loading: boolean;
	saving: boolean;
	error: string | null;
}

const defaultPolicy: ElevationPolicy = {
	mode: 'native',
	locked: false
};

function createElevationStore() {
	const { subscribe, update } = writable<ElevationState>({
		policy: defaultPolicy,
		loading: false,
		saving: false,
		error: null
	});

	return {
		subscribe,
		async load() {
			update((state) => ({ ...state, loading: true, error: null }));
			try {
				const policy = await getElevationPolicy();
				update((state) => ({ ...state, policy, loading: false }));
			} catch (error) {
				update((state) => ({
					...state,
					loading: false,
					error: parseError(error).message
				}));
			}
		},
		async setMode(mode: ElevationMode): Promise<boolean> {
			update((state) => ({ ...state, saving: true, error: null }));
			try {
				const policy = await setElevationMode(mode);
				update((state) => ({ ...state, policy, saving: false }));
				return true;
			} catch (error) {
				update((state) => ({
					...state,
					saving: false,
					error: parseError(error).message
				}));
				return false;
			}
		},
		clearError() {
			update((state) => ({ ...state, error: null }));
		}
	};
}

export const elevation = createElevationStore();
