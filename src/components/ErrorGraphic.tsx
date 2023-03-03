import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { IconDefinition } from "@fortawesome/free-solid-svg-icons"

interface ErrorGraphicProps {
  iconProp: IconDefinition;
  message: string;
}

export default function ErrorGraphic({ iconProp, message }: ErrorGraphicProps) {
    return (
      <div className="flex h-full w-full grow flex-col items-center justify-center gap-4 rounded-t-lg border-b border-gray-faded/30 bg-gray-800">
        <FontAwesomeIcon
          icon={iconProp}
          className="text-title text-gray-400"
        />
        <p className="text-center text-h3 font-medium text-white/50">
          {message}
        </p>
      </div>
    )
}