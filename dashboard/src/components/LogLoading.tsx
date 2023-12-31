import clsx from 'clsx';
export default function LogLoading(
    {loadingText = ""}:
    {loadingText: string}
) {
    return (
        <div className = {clsx(
            "flex flex-row items-center gap-x-1.5",
            "rounded-md py-1 px-2",
        )}>
            <div className="w-4 h-4 ">
            <div
            className="top-0 left-0 h-full w-full animate-spin rounded-full border-4 border-t-4"
            style={{
                borderBottomColor: 'transparent',
                color: '#59B2F3'
            }}
            ></div>
            </div>
            
            <p className="grow truncate text-left italic">{loadingText}</p>
        </div>

    );
}