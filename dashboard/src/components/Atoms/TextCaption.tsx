import { LabelColor } from "./Label";


// note this is very similar to ClipboardTextfield, but without the icon
export default function TextCaption({
    text,
    className
}: {
    text: string;
    className?: string;
}) {
    return (
        <div className = {`group flex flex-row items-center justify-center gap-2 whitespace-nowrap ${className} select-none `}>
            {text}
        </div>
    );
}
