// IDs keep arbitrary subvolume paths out of the Linux mount-options string.
export function withBtrfsSubvolume(options: string, selection: string, readOnly: boolean): string {
	const parts = options.split(',').map((part) => part.trim()).filter(Boolean)
		.filter((part) => !/^(subvol|subvolid)=/.test(part));
	if (selection) parts.push(selection);
	if (readOnly) {
		const filtered = parts.filter((part) => part !== 'rw');
		if (!filtered.includes('ro')) filtered.push('ro');
		return filtered.join(',');
	}
	return parts.join(',');
}
