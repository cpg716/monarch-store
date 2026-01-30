export interface AlpmProgressEvent {
    event_type: string;
    package?: string;
    percent?: number;
    downloaded?: number;
    total?: number;
    message: string;
}

export type AlpmEventType = 
    | 'progress'
    | 'download_progress'
    | 'download_start'
    | 'download_complete'
    | 'extract_start'
    | 'extract_progress'
    | 'extract_complete'
    | 'install_start'
    | 'install_progress'
    | 'install_complete'
    | 'package_found'
    | 'package_marked'
    | 'file_added'
    | 'transaction_complete'
    | 'error';
