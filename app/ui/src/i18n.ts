// EN + BN strings (file 04 §14). Keys returned by the core (message_key / error codes) resolve here too.
import { create } from 'zustand';

export type Lang = 'en' | 'bn';
type Dict = Record<string, string>;

const en: Dict = {
  'nav.profiles': 'Profiles', 'nav.proxies': 'Proxies', 'nav.logs': 'Activity log', 'nav.settings': 'Settings', 'nav.trash': 'Trash',
  'nav.folders': 'Folders', 'nav.tags': 'Tags', 'nav.all': 'All profiles', 'nav.new_folder': 'New folder', 'nav.new_tag': 'New tag',
  'vault.welcome': 'Welcome to Vanta', 'vault.welcome_sub': 'Create a master password. It encrypts every profile, proxy and cookie on this computer. Nothing leaves this machine.',
  'vault.unlock_title': 'Unlock Vanta', 'vault.unlock': 'Unlock', 'vault.create': 'Create vault', 'vault.password': 'Master password',
  'vault.confirm': 'Confirm password', 'vault.hint': 'Hint (optional, stored unencrypted)', 'vault.lock': 'Lock',
  'vault.mismatch': 'Passwords do not match.', 'vault.recovery_title': 'Save your recovery code',
  'vault.recovery_note': 'Shown only once. Store it offline. It is the only way back in if you forget the password.',
  'vault.recovery_done': "I've saved it", 'vault.use_recovery': 'Use recovery code', 'vault.use_password': 'Use password',
  'vault.recovery_code': 'Recovery code', 'vault.hint_label': 'Hint: {h}', 'vault.deriving': 'Deriving key…',
  'vault.change_pw': 'Change master password', 'vault.old_pw': 'Current password', 'vault.new_pw': 'New password',
  'profiles.new': 'New profile', 'profiles.bulk': 'Bulk create', 'profiles.search': 'Search name, notes, tags…',
  'profiles.launch': 'Launch', 'profiles.stop': 'Stop', 'profiles.trash': 'Move to trash', 'profiles.restore': 'Restore',
  'profiles.empty': 'No profiles yet. Create your first one.', 'profiles.total': '{n} profiles', 'profiles.selected': '{n} selected',
  'profiles.select_all': 'Select all {n}', 'profiles.clear': 'Clear', 'profiles.edit': 'Edit', 'profiles.pin': 'Pin', 'profiles.unpin': 'Unpin',
  'profiles.move_to': 'Move to folder…', 'profiles.no_folder': 'No folder',
  'col.name': 'Name', 'col.status': 'Status', 'col.os': 'System', 'col.proxy': 'Proxy', 'col.tags': 'Tags', 'col.last': 'Last launch',
  'editor.create_title': 'New profile', 'editor.edit_title': 'Edit profile', 'editor.general': 'General', 'editor.network': 'Network', 'editor.identity': 'Identity',
  'editor.name': 'Name', 'editor.folder': 'Folder', 'editor.tags': 'Tags', 'editor.proxy': 'Proxy', 'editor.none': 'None (direct — not recommended)',
  'editor.geo': 'Timezone & language', 'geo.auto_from_proxy': 'Match proxy exit (recommended)', 'geo.manual': 'Set manually', 'geo.host': 'Same as this computer',
  'editor.timezone': 'Timezone (IANA, e.g. Asia/Dhaka)', 'editor.languages': 'Languages (comma separated)',
  'editor.webrtc': 'WebRTC', 'webrtc.proxied': 'Through proxy only (no leak)', 'webrtc.public_only': 'Public interface only', 'webrtc.disabled_present': 'Blocked', 'webrtc.real': 'Real (leaks real IP)',
  'editor.block_mismatch': 'Block launch if timezone/language do not match the proxy',
  'editor.ram': 'Memory cap (MB, empty = none)', 'editor.save': 'Save', 'editor.cancel': 'Cancel', 'editor.create': 'Create',
  'editor.start_urls': 'Start pages (one per line)', 'editor.notes': 'Notes', 'editor.resolved': 'What websites will see',
  'editor.native_note': 'Vanta keeps each profile on this computer\'s real OS and GPU family so rendering stays coherent. Per-profile values below are derived from the profile\'s own secret.',
  'bulk.title': 'Bulk create profiles', 'bulk.count': 'How many (1–10,000)', 'bulk.pattern': 'Name pattern ({n} or {n:0000})', 'bulk.start': 'Start number',
  'bulk.create': 'Create {n}', 'bulk.done': 'Created {n} profiles', 'bulk.working': 'Creating…',
  'proxies.title': 'Proxy hub', 'proxies.add': 'Add proxy', 'proxies.import': 'Import list', 'proxies.check': 'Check', 'proxies.check_all': 'Check all',
  'proxies.empty': 'No proxies yet.', 'proxies.paste': 'One per line: host:port · host:port:user:pass · user:pass@host:port · socks5://user:pass@host:port',
  'proxies.default_kind': 'Type for lines without scheme', 'proxies.preview': '{ok} valid · {bad} invalid', 'proxies.imported': 'Imported {n} proxies',
  'proxies.kind': 'Type', 'proxies.host': 'Host', 'proxies.port': 'Port', 'proxies.user': 'Username', 'proxies.pass': 'Password', 'proxies.label': 'Label',
  'proxies.exit': 'Exit', 'proxies.latency': 'Latency', 'proxies.used_by': 'Profiles', 'proxies.delete': 'Delete',
  'pstatus.ok': 'OK', 'pstatus.slow': 'Slow', 'pstatus.fail': 'Failed', 'pstatus.auth_fail': 'Auth failed', 'pstatus.unknown': 'Not checked',
  'status.idle': 'Idle', 'status.queued': 'Queued', 'status.running': 'Running', 'status.starting': 'Starting', 'status.crashed': 'Crashed', 'status.stopping': 'Stopping', 'status.locked': 'Locked',
  'settings.title': 'Settings', 'settings.engine': 'Browser engine', 'settings.engine_path': 'Engine path (chrome.exe)', 'settings.engine_ok': 'Engine found',
  'settings.engine_missing': 'Engine not found at this path', 'settings.language': 'Language', 'settings.theme': 'Theme', 'theme.dark': 'Dark', 'theme.light': 'Light',
  'settings.concurrency': 'Max profiles running at once (empty = automatic)', 'settings.autolock': 'Auto-lock after idle minutes (empty = off)',
  'settings.host': 'This computer', 'settings.security': 'Security', 'settings.saved': 'Saved', 'settings.echo': 'Proxy check endpoint',
  'logs.title': 'Activity log', 'logs.empty': 'Nothing logged yet.',
  'launch.started': 'Started', 'launch.engine_missing': 'Engine not installed. Set chrome.exe path in Settings.',
  'launch.limit': 'Concurrency limit reached', 'launch.already_running': 'Already running', 'launch.spawn_failed': 'Could not start the engine.',
  'launch.gateway_failed': 'Could not open the local proxy gateway.', 'launch.proxy_kind_unsupported': 'This proxy type is not supported yet.',
  'launch.failed': 'Launch failed', 'proxy.connect_failed': 'Could not connect through the proxy.', 'proxy.auth_failed': 'Proxy rejected the credentials.',
  'proxy.timeout': 'Proxy timed out.', 'proxy.invalid_host': 'Invalid host.', 'proxy.invalid_port': 'Invalid port.', 'proxy.missing_port': 'Missing port.',
  'proxy.unrecognised_format': 'Unrecognised format.', 'proxy.unsupported_scheme': 'Unsupported scheme.', 'proxy.echo_failed': 'Exit lookup failed.',
  'fp.timezone_mismatch': 'Timezone does not match the proxy exit.', 'fp.timezone_invalid': 'Timezone is not a valid IANA name.',
  'fp.render_class': 'Fingerprint was made on a different computer (GPU class changed). Regenerate it.', 'fp.webrtc_leak': 'WebRTC "real" with a proxy would leak your IP.',
  'err.vault_locked': 'Vault is locked.', 'err.wrong_password': 'Wrong password.', 'err.retry_later': 'Too many attempts. Wait {n}s.',
  'err.not_found': 'Not found.', 'err.conflict': 'Already exists.', 'err.validation': 'Please check the highlighted fields.',
  'err.internal': 'Something went wrong.', 'err.io': 'Disk error.', 'err.integrity': 'Integrity check failed — data may be tampered.',
  'err.limit': 'Limit exceeded.', 'vault.password_too_short': 'Password must be at least 10 characters.',
  'common.close': 'Close', 'common.confirm_trash': 'Move {n} profiles to trash?', 'common.yes': 'Yes', 'common.no': 'No', 'common.refresh': 'Refresh',
};

const bn: Dict = {
  'nav.profiles': 'প্রোফাইল', 'nav.proxies': 'প্রক্সি', 'nav.logs': 'কার্যকলাপ লগ', 'nav.settings': 'সেটিংস', 'nav.trash': 'ট্র্যাশ',
  'nav.folders': 'ফোল্ডার', 'nav.tags': 'ট্যাগ', 'nav.all': 'সব প্রোফাইল', 'nav.new_folder': 'নতুন ফোল্ডার', 'nav.new_tag': 'নতুন ট্যাগ',
  'vault.welcome': 'Vanta-তে স্বাগতম', 'vault.welcome_sub': 'একটি মাস্টার পাসওয়ার্ড দিন। এটি এই কম্পিউটারের প্রতিটি প্রোফাইল, প্রক্সি আর কুকি এনক্রিপ্ট করে। কিছুই এই মেশিনের বাইরে যায় না।',
  'vault.unlock_title': 'Vanta আনলক করুন', 'vault.unlock': 'আনলক', 'vault.create': 'ভল্ট তৈরি করুন', 'vault.password': 'মাস্টার পাসওয়ার্ড',
  'vault.confirm': 'পাসওয়ার্ড আবার দিন', 'vault.hint': 'হিন্ট (ঐচ্ছিক, এনক্রিপ্ট ছাড়া রাখা হয়)', 'vault.lock': 'লক',
  'vault.mismatch': 'পাসওয়ার্ড দুটো মেলেনি।', 'vault.recovery_title': 'রিকভারি কোড সংরক্ষণ করুন',
  'vault.recovery_note': 'এটি একবারই দেখানো হবে। অফলাইনে রাখুন। পাসওয়ার্ড ভুলে গেলে ফেরার একমাত্র উপায় এটি।',
  'vault.recovery_done': 'সংরক্ষণ করেছি', 'vault.use_recovery': 'রিকভারি কোড দিয়ে খুলুন', 'vault.use_password': 'পাসওয়ার্ড দিয়ে খুলুন',
  'vault.recovery_code': 'রিকভারি কোড', 'vault.hint_label': 'হিন্ট: {h}', 'vault.deriving': 'কী তৈরি হচ্ছে…',
  'vault.change_pw': 'মাস্টার পাসওয়ার্ড বদলান', 'vault.old_pw': 'বর্তমান পাসওয়ার্ড', 'vault.new_pw': 'নতুন পাসওয়ার্ড',
  'profiles.new': 'নতুন প্রোফাইল', 'profiles.bulk': 'একসাথে তৈরি', 'profiles.search': 'নাম, নোট, ট্যাগ খুঁজুন…',
  'profiles.launch': 'চালু', 'profiles.stop': 'বন্ধ', 'profiles.trash': 'ট্র্যাশে পাঠান', 'profiles.restore': 'ফিরিয়ে আনুন',
  'profiles.empty': 'এখনও কোনো প্রোফাইল নেই। প্রথমটি তৈরি করুন।', 'profiles.total': '{n}টি প্রোফাইল', 'profiles.selected': '{n}টি নির্বাচিত',
  'profiles.select_all': 'সব {n}টি নির্বাচন', 'profiles.clear': 'বাদ দিন', 'profiles.edit': 'এডিট', 'profiles.pin': 'পিন', 'profiles.unpin': 'আনপিন',
  'profiles.move_to': 'ফোল্ডারে সরান…', 'profiles.no_folder': 'কোনো ফোল্ডার নয়',
  'col.name': 'নাম', 'col.status': 'অবস্থা', 'col.os': 'সিস্টেম', 'col.proxy': 'প্রক্সি', 'col.tags': 'ট্যাগ', 'col.last': 'শেষ চালু',
  'editor.create_title': 'নতুন প্রোফাইল', 'editor.edit_title': 'প্রোফাইল এডিট', 'editor.general': 'সাধারণ', 'editor.network': 'নেটওয়ার্ক', 'editor.identity': 'পরিচয়',
  'editor.name': 'নাম', 'editor.folder': 'ফোল্ডার', 'editor.tags': 'ট্যাগ', 'editor.proxy': 'প্রক্সি', 'editor.none': 'কোনোটি নয় (সরাসরি — সুপারিশ করা হয় না)',
  'editor.geo': 'টাইমজোন ও ভাষা', 'geo.auto_from_proxy': 'প্রক্সি এক্সিটের সাথে মিলিয়ে (সুপারিশকৃত)', 'geo.manual': 'নিজে ঠিক করুন', 'geo.host': 'এই কম্পিউটারের মতো',
  'editor.timezone': 'টাইমজোন (IANA, যেমন Asia/Dhaka)', 'editor.languages': 'ভাষা (কমা দিয়ে)',
  'editor.webrtc': 'WebRTC', 'webrtc.proxied': 'শুধু প্রক্সি দিয়ে (লিক নেই)', 'webrtc.public_only': 'শুধু পাবলিক ইন্টারফেস', 'webrtc.disabled_present': 'বন্ধ', 'webrtc.real': 'আসল (আসল IP লিক হয়)',
  'editor.block_mismatch': 'টাইমজোন/ভাষা প্রক্সির সাথে না মিললে চালু হবে না',
  'editor.ram': 'মেমরি সীমা (MB, খালি = নেই)', 'editor.save': 'সংরক্ষণ', 'editor.cancel': 'বাতিল', 'editor.create': 'তৈরি',
  'editor.start_urls': 'শুরুর পেজ (প্রতি লাইনে একটি)', 'editor.notes': 'নোট', 'editor.resolved': 'ওয়েবসাইট যা দেখবে',
  'editor.native_note': 'রেন্ডারিং যেন সামঞ্জস্যপূর্ণ থাকে, তাই প্রতিটি প্রোফাইল এই কম্পিউটারের আসল OS ও GPU পরিবারেই থাকে। নিচের মানগুলো প্রোফাইলের নিজস্ব গোপন কী থেকে তৈরি।',
  'bulk.title': 'একসাথে প্রোফাইল তৈরি', 'bulk.count': 'কতগুলো (১–১০,০০০)', 'bulk.pattern': 'নামের প্যাটার্ন ({n} বা {n:0000})', 'bulk.start': 'শুরুর সংখ্যা',
  'bulk.create': '{n}টি তৈরি করুন', 'bulk.done': '{n}টি প্রোফাইল তৈরি হয়েছে', 'bulk.working': 'তৈরি হচ্ছে…',
  'proxies.title': 'প্রক্সি হাব', 'proxies.add': 'প্রক্সি যোগ', 'proxies.import': 'তালিকা ইমপোর্ট', 'proxies.check': 'পরীক্ষা', 'proxies.check_all': 'সব পরীক্ষা',
  'proxies.empty': 'এখনও কোনো প্রক্সি নেই।', 'proxies.paste': 'প্রতি লাইনে একটি: host:port · host:port:user:pass · user:pass@host:port · socks5://user:pass@host:port',
  'proxies.default_kind': 'স্কিম ছাড়া লাইনের ধরন', 'proxies.preview': '{ok}টি ঠিক · {bad}টি ভুল', 'proxies.imported': '{n}টি প্রক্সি ইমপোর্ট হয়েছে',
  'proxies.kind': 'ধরন', 'proxies.host': 'হোস্ট', 'proxies.port': 'পোর্ট', 'proxies.user': 'ইউজারনেম', 'proxies.pass': 'পাসওয়ার্ড', 'proxies.label': 'লেবেল',
  'proxies.exit': 'এক্সিট', 'proxies.latency': 'লেটেন্সি', 'proxies.used_by': 'প্রোফাইল', 'proxies.delete': 'মুছুন',
  'pstatus.ok': 'ঠিক আছে', 'pstatus.slow': 'ধীর', 'pstatus.fail': 'ব্যর্থ', 'pstatus.auth_fail': 'লগইন ব্যর্থ', 'pstatus.unknown': 'পরীক্ষা হয়নি',
  'status.idle': 'নিষ্ক্রিয়', 'status.queued': 'অপেক্ষায়', 'status.running': 'চলছে', 'status.starting': 'শুরু হচ্ছে', 'status.crashed': 'ক্র্যাশ', 'status.stopping': 'বন্ধ হচ্ছে', 'status.locked': 'লক',
  'settings.title': 'সেটিংস', 'settings.engine': 'ব্রাউজার ইঞ্জিন', 'settings.engine_path': 'ইঞ্জিনের পাথ (chrome.exe)', 'settings.engine_ok': 'ইঞ্জিন পাওয়া গেছে',
  'settings.engine_missing': 'এই পাথে ইঞ্জিন নেই', 'settings.language': 'ভাষা', 'settings.theme': 'থিম', 'theme.dark': 'ডার্ক', 'theme.light': 'লাইট',
  'settings.concurrency': 'একসাথে সর্বোচ্চ প্রোফাইল (খালি = স্বয়ংক্রিয়)', 'settings.autolock': 'এত মিনিট নিষ্ক্রিয় থাকলে অটো-লক (খালি = বন্ধ)',
  'settings.host': 'এই কম্পিউটার', 'settings.security': 'নিরাপত্তা', 'settings.saved': 'সংরক্ষিত', 'settings.echo': 'প্রক্সি চেকের ঠিকানা',
  'logs.title': 'কার্যকলাপ লগ', 'logs.empty': 'এখনও কিছু লগ হয়নি।',
  'launch.started': 'চালু হয়েছে', 'launch.engine_missing': 'ইঞ্জিন ইনস্টল নেই। সেটিংসে chrome.exe-এর পাথ দিন।',
  'launch.limit': 'একসাথে চালানোর সীমা পূর্ণ', 'launch.already_running': 'আগে থেকেই চলছে', 'launch.spawn_failed': 'ইঞ্জিন চালু করা গেল না।',
  'launch.gateway_failed': 'লোকাল প্রক্সি গেটওয়ে খোলা গেল না।', 'launch.proxy_kind_unsupported': 'এই ধরনের প্রক্সি এখনও সমর্থিত নয়।',
  'launch.failed': 'চালু ব্যর্থ', 'proxy.connect_failed': 'প্রক্সি দিয়ে সংযোগ হলো না।', 'proxy.auth_failed': 'প্রক্সি ইউজারনেম/পাসওয়ার্ড ভুল।',
  'proxy.timeout': 'প্রক্সি টাইমআউট।', 'proxy.invalid_host': 'হোস্ট ভুল।', 'proxy.invalid_port': 'পোর্ট ভুল।', 'proxy.missing_port': 'পোর্ট নেই।',
  'proxy.unrecognised_format': 'ফরম্যাট চেনা যায়নি।', 'proxy.unsupported_scheme': 'স্কিম সমর্থিত নয়।', 'proxy.echo_failed': 'এক্সিট তথ্য পাওয়া যায়নি।',
  'fp.timezone_mismatch': 'টাইমজোন প্রক্সি এক্সিটের সাথে মেলে না।', 'fp.timezone_invalid': 'টাইমজোন সঠিক IANA নাম নয়।',
  'fp.render_class': 'ফিঙ্গারপ্রিন্ট অন্য কম্পিউটারে তৈরি (GPU বদলেছে)। নতুন করে তৈরি করুন।', 'fp.webrtc_leak': 'প্রক্সির সাথে WebRTC "আসল" রাখলে IP লিক হবে।',
  'err.vault_locked': 'ভল্ট লক করা।', 'err.wrong_password': 'ভুল পাসওয়ার্ড।', 'err.retry_later': 'অনেকবার চেষ্টা হয়েছে। {n} সেকেন্ড অপেক্ষা করুন।',
  'err.not_found': 'পাওয়া যায়নি।', 'err.conflict': 'আগে থেকেই আছে।', 'err.validation': 'চিহ্নিত ঘরগুলো দেখুন।',
  'err.internal': 'কিছু একটা সমস্যা হয়েছে।', 'err.io': 'ডিস্কে সমস্যা।', 'err.integrity': 'অখণ্ডতা যাচাই ব্যর্থ — ডেটায় কারসাজি হতে পারে।',
  'err.limit': 'সীমা ছাড়িয়েছে।', 'vault.password_too_short': 'পাসওয়ার্ড অন্তত ১০ অক্ষরের হতে হবে।',
  'common.close': 'বন্ধ', 'common.confirm_trash': '{n}টি প্রোফাইল ট্র্যাশে পাঠাবেন?', 'common.yes': 'হ্যাঁ', 'common.no': 'না', 'common.refresh': 'রিফ্রেশ',
};

export const dicts: Record<Lang, Dict> = { en, bn };

interface I18n { lang: Lang; setLang: (l: Lang) => void; t: (key: string, vars?: Record<string, string | number>) => string }

export const useI18n = create<I18n>((set, get) => ({
  lang: (localStorage.getItem('vanta.lang') as Lang) || 'bn',
  setLang: (lang) => { localStorage.setItem('vanta.lang', lang); document.documentElement.lang = lang; set({ lang }); },
  t: (key, vars) => {
    let s = dicts[get().lang][key] ?? en[key] ?? key;
    if (vars) for (const k of Object.keys(vars)) s = s.split(`{${k}}`).join(String(vars[k]));
    return s;
  },
}));

/** Human message for an error coming from the core. */
export function errText(t: I18n['t'], e: unknown): string {
  const err = e && typeof e === 'object' && 'code' in (e as any) ? (e as any) : { code: 'internal' };
  if (err.code === 'retry_later') return t('err.retry_later', { n: err.detail });
  if (err.code === 'validation' && Array.isArray(err.detail) && err.detail[0]?.messageKey) return t(err.detail[0].messageKey);
  if ((err.code === 'launch' || err.code === 'proxy') && typeof err.detail === 'string') return t(err.detail);
  return t(`err.${err.code}`);
}
